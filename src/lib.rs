// #![windows_subsystem = "console"]
#![windows_subsystem = "windows"]
use std::sync::Arc;
// #![allow(warnings)]
use std::{env, sync::mpsc::Receiver, time::Duration};
// winit's event loop uses web-time's Instant on wasm (WaitUntil, timers), so
// alias Instant to the matching type per target to keep frame-pacing types aligned.
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use crate::log::LogType;
#[cfg(feature = "headed")]
use crate::controls::bit_check;
#[cfg(not(target_arch = "wasm32"))]
#[cfg(desktop)]
use clipboard::{ClipboardContext, ClipboardProvider};
use colored::Colorize;
#[cfg(feature = "headed")]
use ent_manager::InstanceBuffer;
use glam::vec2;
use global::StateChange;
use image::GenericImageView;
use itertools::Itertools;
use lua_define::{LuaResponse, MainPacket};
use pollster::block_on;
use root::Core;
use types::{ControlState, GlobalMap};

mod asset;
mod bundle;
mod command;
#[cfg(feature = "headed")]
mod controls;
#[cfg(feature = "headed")]
mod ent;
mod ent_manager;
mod error;
mod file_util;
#[cfg(feature = "headed")]
mod gfx;
mod global;
mod gui;
mod log;
#[cfg(feature = "online_capable")]
mod lua_connection;
mod lua_define;
mod lua_ent;
mod lua_img;
mod model;
#[cfg(feature = "online_capable")]
mod online;
#[cfg(feature = "online_capable")]
mod packet;
mod pad;
mod parse;
mod pool;
#[cfg(feature = "headed")]
mod post;
#[cfg(feature = "headed")]
mod ray;
#[cfg(feature = "headed")]
mod render;
mod root;
#[cfg(feature = "audio")]
mod fx;
#[cfg(feature = "audio")]
mod sound;
// MIDI input is native-only for now (midir has no wasm path wired up here).
#[cfg(all(feature = "midi", not(target_arch = "wasm32")))]
mod midi;
#[cfg(feature = "audio")]
mod vocaloid;
mod template;
mod texture;
mod tile;
#[cfg(all(feature = "render-tui", not(target_arch = "wasm32")))]
mod tui;
mod types;

#[cfg(all(feature = "headed", feature = "render-tui"))]
compile_error!(
    "features `headed` and `render-tui` are mutually exclusive — pick one renderer backend"
);
#[cfg(all(feature = "headed", target_arch = "wasm32"))]
mod web_worker;
#[cfg(target_arch = "wasm32")]
mod worker;
mod worker_protocol;
#[cfg(feature = "picc")]
mod userdata_util;
mod world;

use command::MainCommmand;
#[cfg(feature = "headed")]
use winit::error::EventLoopError;
#[cfg(feature = "headed")]
use winit::{
    application::ApplicationHandler,
    dpi::LogicalPosition,
    event::{DeviceEvent, DeviceId, ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    window::{CursorGrabMode, Window, WindowAttributes, WindowId},
};

#[cfg(target_os = "windows")]
const OS: &str = "win";

#[cfg(target_os = "linux")]
const OS: &str = "nix";

#[cfg(target_os = "macos")]
const OS: &str = "mac";

#[cfg(target_arch = "wasm32")]
const OS: &str = "web";

// Visible to Lua, so games can adapt (touch-sized hit targets, no keyboard).
#[cfg(target_os = "android")]
const OS: &str = "droid";

#[cfg(target_os = "ios")]
const OS: &str = "ios";

// Anything else native: better a build that runs and reports an odd name than one
// that won't compile because a platform wasn't foreseen here.
#[cfg(not(any(
    target_os = "windows",
    target_os = "linux",
    target_os = "macos",
    target_os = "android",
    target_os = "ios",
    target_arch = "wasm32"
)))]
const OS: &str = "other";

const FPS: f32 = 60.;

/// Per-loop output of `Core::update`: the freshly-built entity instance buffers
/// under `headed`, nothing headless.
#[cfg(feature = "headed")]
type UpdateOut = InstanceBuffer;
#[cfg(not(feature = "headed"))]
type UpdateOut = ();

#[cfg(feature = "headed")]
pub struct App {
    window: Option<Arc<Window>>,
    center: LogicalPosition<f64>,
    core: Option<Core>,
    next_frame_time: Instant,
    frame_duration: Duration,
    catcher: Option<Receiver<MainPacket>>,
    /// Current key/mouse-button state sent to Lua each frame.
    bits: ControlState,
    /// Key state from the previous Lua frame — used for pressed/released detection.
    bits_prev: [bool; 256],
    /// A game named on the command line (or found by auto-load), applied on the
    /// first frame rather than during `resumed`. Unpacking a game is slow enough
    /// that doing it while the window is being created stops the OS activating it.
    #[cfg(not(target_arch = "wasm32"))]
    deferred_load: Option<String>,
    /// wasm builds init the engine asynchronously (wgpu adapter/device requests
    /// can't block the browser main thread). `resumed` kicks off the build via
    /// spawn_local and drops the finished Core here; the frame loop installs it
    /// once ready. Rc<RefCell<…>> is fine — wasm is single-threaded.
    #[cfg(target_arch = "wasm32")]
    pending_core: std::rc::Rc<std::cell::RefCell<Option<(Core, Receiver<MainPacket>)>>>,
    /// The VM web worker (wasm only). Spawned once; drives the Lua VM off-thread.
    #[cfg(target_arch = "wasm32")]
    worker: Option<crate::web_worker::WorkerHandle>,
    /// `--overlay <path>` from the command line: bring a tool up once the app is
    /// running. Engine-level by nature (whoever launched the binary is trusted), and
    /// deliberately not reachable from the app — same rule as the console command.
    #[cfg(not(target_arch = "wasm32"))]
    deferred_overlay: Option<String>,
    /// Set while the native window (and therefore the surface) is gone — Android
    /// tears it down whenever the app leaves the foreground. Named for what it means
    /// rather than "suspended", which is already a method on this type.
    surface_lost: bool,
    /// Set at boot when the game is compiled in (`include_auto`); the load itself
    /// happens on the first frame, so window activation isn't blocked by unpacking.
    #[cfg(all(not(target_arch = "wasm32"), feature = "include_auto"))]
    deferred_auto: bool,
    /// The finger acting as the cursor, by winit touch id — see the `Touch` arm.
    /// `None` when nothing is touching. Not cfg'd to mobile: desktop touchscreens
    /// send these events too.
    primary_touch: Option<u64>,
    /// Last primary-touch position in window pixels, for computing drag deltas
    /// (touch has no equivalent of DeviceEvent::MouseMotion).
    touch_last: Option<(f64, f64)>,
    /// Whether the worker has been sent its Init + initial Load.
    #[cfg(target_arch = "wasm32")]
    worker_inited: bool,
    /// Whether the worker has been told how its sound commands travel — given the
    /// worklet's command lane, or told none is coming. Settled once per session.
    #[cfg(all(target_arch = "wasm32", feature = "audio"))]
    sound_lane_settled: bool,
    /// Shared "app wants the mouse grabbed" flag (mirrors global.mouse_grab).
    /// The canvas mousedown handler reads it to decide whether to request
    /// pointer-lock (which browsers only grant from a user gesture).
    #[cfg(target_arch = "wasm32")]
    pointer_lock_wanted: std::rc::Rc<std::cell::RefCell<bool>>,
    /// The web AudioContext (a JS reference), shared so a DOM gesture handler can
    /// resume it — browsers start it suspended until the user interacts.
    /// Populated when the async Core is installed.
    #[cfg(all(target_arch = "wasm32", feature = "audio"))]
    audio_ctx: std::rc::Rc<std::cell::RefCell<Option<web_sys::AudioContext>>>,
    /// The game bundle, unzipped asynchronously (fetch or embedded). None until
    /// ready; the frame loop routes it to the worker/GPU/audio once present.
    #[cfg(target_arch = "wasm32")]
    pending_bundle: std::rc::Rc<std::cell::RefCell<Option<WasmBundle>>>,
}

#[cfg(feature = "headed")]
impl Default for App {
    fn default() -> Self {
        Self {
            window: None,
            center: LogicalPosition::new(320.0f64, 240.0f64),
            core: None,
            next_frame_time: Instant::now(),
            frame_duration: Duration::from_secs_f32(1.0 / FPS),
            catcher: None,
            bits: ControlState::default(),
            bits_prev: [false; 256],
            #[cfg(not(target_arch = "wasm32"))]
            deferred_load: None,
            #[cfg(target_arch = "wasm32")]
            pending_core: std::rc::Rc::new(std::cell::RefCell::new(None)),
            #[cfg(target_arch = "wasm32")]
            worker: None,
            #[cfg(all(not(target_arch = "wasm32"), feature = "include_auto"))]
            deferred_auto: false,
            #[cfg(not(target_arch = "wasm32"))]
            deferred_overlay: None,
            surface_lost: false,
            primary_touch: None,
            touch_last: None,
            #[cfg(target_arch = "wasm32")]
            worker_inited: false,
            #[cfg(all(target_arch = "wasm32", feature = "audio"))]
            sound_lane_settled: false,
            #[cfg(target_arch = "wasm32")]
            pointer_lock_wanted: std::rc::Rc::new(std::cell::RefCell::new(false)),
            #[cfg(all(target_arch = "wasm32", feature = "audio"))]
            audio_ctx: std::rc::Rc::new(std::cell::RefCell::new(None)),
            #[cfg(target_arch = "wasm32")]
            pending_bundle: std::rc::Rc::new(std::cell::RefCell::new(None)),
        }
    }
}

#[cfg(feature = "headed")]
fn state_change_checker(
    c: &mut Core,
    control_flow: &ActiveEventLoop,
    rwindow: &Arc<winit::window::Window>,
    center: LogicalPosition<f64>,
) -> bool {
    if c.global.is_state_changed {
        if c.global.state_delay > 0 {
            c.global.state_delay -= 1;
        } else {
            c.global.is_state_changed = false;
            let states: Vec<StateChange> = c.global.state_changes.drain(..).collect();
            for state in states {
                match state {
                    #[cfg(feature = "headed")]
                    StateChange::MouseGrabOn => {
                        rwindow.set_cursor_visible(false);
                        let _ = rwindow.set_cursor_position(center);
                        rwindow
                            .set_cursor_grab(CursorGrabMode::Confined)
                            .or_else(|_| rwindow.set_cursor_grab(CursorGrabMode::Locked))
                            .ok();
                        c.global.mouse_grabbed_state = true;
                    }
                    #[cfg(feature = "headed")]
                    StateChange::MouseGrabOff => {
                        rwindow.set_cursor_visible(true);
                        let _ = rwindow.set_cursor_grab(CursorGrabMode::None);
                        c.global.mouse_grabbed_state = false;
                    }
                    #[cfg(feature = "headed")]
                    StateChange::Resized => {
                        c.debounced_resize();
                    }
                    #[cfg(feature = "headed")]
                    StateChange::Quit => control_flow.exit(),
                    #[cfg(not(feature = "headed"))]
                    StateChange::Quit => return false,
                    StateChange::Config => {
                        let res = crate::asset::parse_config(
                            &mut c.global,
                            c.bundle_manager.get_lua(),
                            &mut c.loggy,
                        );
                        if let Some(s) = res {
                            let _ = crate::command::run_con_sys(c, &s);
                        }

                        // Also check command-line arguments here.
                        let args: Vec<String> = std::env::args().collect();
                        if args.len() > 1 {
                            let mut command = None;
                            for (_i, arg) in args.iter().enumerate() {
                                if arg.starts_with('-') {
                                    command = Some(arg.to_lowercase());
                                } else if command.is_some() {
                                    match command.unwrap().as_str() {
                                        "--init" | "-i" => {
                                            println!("cli-init: {:?}", arg);
                                            crate::command::run_con_sys(c, arg);
                                        }
                                        "--new" | "-n" => {
                                            println!("cli-new: {:?}", arg);
                                            let _ = crate::command::run_con_sys(
                                                c,
                                                &format!("new {}", arg),
                                            );
                                        }
                                        _ => {}
                                    }
                                    command = None;
                                }
                            }
                        }
                    }
                    StateChange::ModelChange(id) => {
                        #[cfg(feature = "headed")]
                        c.ent_manager.check_for_model_change(&c.model_manager, &id);
                    }
                }
            }
            #[cfg(feature = "headed")]
            c.check_fullscreen();
        }
    }
    // Reconcile the app's desired mouse grab (mouse_grab, set via `mgrab`) with
    // the actual cursor state each frame. Native grabs the cursor directly; the
    // web build defers to a canvas click (via pointer_lock_wanted), so this path
    // is gated off wasm. Released while the console is open.
    #[cfg(all(feature = "headed", not(target_arch = "wasm32")))]
    {
        let want = c.global.mouse_grab && !c.global.console;
        if want != c.global.mouse_grabbed_state {
            if want {
                rwindow.set_cursor_visible(false);
                let _ = rwindow.set_cursor_position(center);
                rwindow
                    .set_cursor_grab(CursorGrabMode::Confined)
                    .or_else(|_| rwindow.set_cursor_grab(CursorGrabMode::Locked))
                    .ok();
            } else {
                rwindow.set_cursor_visible(true);
                let _ = rwindow.set_cursor_grab(CursorGrabMode::None);
            }
            c.global.mouse_grabbed_state = want;
        }
    }
    false
}

#[cfg(feature = "headed")]
impl ApplicationHandler for App {
    /// Called when the event loop is ready and (on mobile/web) the app has resumed.
    /// This is where we create the window and initialise the engine.
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            // Already built — but `resumed` is not a once-per-launch event on
            // mobile. Android destroys the native window whenever the app leaves
            // the foreground (locking the screen is enough) and calls this again
            // with a fresh one on the way back. The surface still points at the
            // window that's gone, so every frame drew nowhere and the app came
            // back black. Rebuild just the surface, keeping the device, the
            // pipelines and the running game.
            if self.surface_lost {
                self.surface_lost = false;
                let size = self.window.as_ref().map(|w| w.inner_size());
                if let Some(core) = self.core.as_mut() {
                    if core.gfx.recreate_surface() {
                        ::log::info!("petrichor64: surface rebuilt after resume");
                        // Re-derive everything sized from the surface: the new
                        // window need not match the old one (rotation, a fold
                        // opening), and `resize` is already the path that rebuilds
                        // the depth texture, post targets and gui scaling.
                        if let Some(size) = size {
                            core.resize(size);
                        }
                    }
                }
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            return;
        }

        // --- Window attributes (icon is native-only; winit ignores it on web) ---
        #[cfg(not(target_arch = "wasm32"))]
        let win_attr = {
            let icon = image::load_from_memory(include_bytes!(
                "../assets/petrichor-small-icon.png"
            ))
            .expect("failed to load icon.png");
            let rgba = icon.as_rgba8().unwrap();
            let (width, height) = icon.dimensions();
            let bytes = rgba
                .chunks_exact(4)
                .flat_map(|p| p.iter().cloned())
                .collect::<Vec<_>>();
            let window_icon = winit::window::Icon::from_rgba(bytes, width, height).unwrap();
            WindowAttributes::default()
                .with_title("Petrichor64")
                .with_inner_size(winit::dpi::LogicalSize::new(640i32, 548i32))
                .with_window_icon(Some(window_icon))
        };
        #[cfg(target_arch = "wasm32")]
        let win_attr = WindowAttributes::default()
            .with_title("Petrichor64")
            .with_inner_size(winit::dpi::LogicalSize::new(640i32, 548i32));

        let window = Arc::new(
            event_loop
                .create_window(win_attr)
                .expect("failed to create window"),
        );
        self.window = Some(window.clone());
        // Ask the OS to bring us forward. Launched from a terminal the window can
        // otherwise open behind it and never take keyboard focus, which reads as
        // "the app ignores all input". A hint only — window managers may refuse it.
        #[cfg(not(target_arch = "wasm32"))]
        window.focus_window();

        // --- Native: build Core synchronously and load the default app. ---
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (mut core, catcher) = pollster::block_on(Core::new(window.clone()));
            self.catcher = Some(catcher);

            // Boot to the built-in "nil" app (embedded nil.game.png via
            // load_empty -> get_logo). A command-line/auto game replaces it
            // below; with no arg, nil is the fallback state.
            crate::command::load_empty(&mut core);
            core.loggy.clear();

            core.global.state_changes.push(StateChange::Config);
            // Small delay so the console-app's pending requests finish before
            // the following config state change fires.
            core.global.state_delay = 8;
            core.global.is_state_changed = true;

            // `--overlay <path>`: applied after the app loads, on the first frame.
            #[cfg(not(target_arch = "wasm32"))]
            {
                let args: Vec<String> = env::args().collect();
                if let Some(i) = args.iter().position(|a| a == "--overlay") {
                    self.deferred_overlay = args.get(i + 1).cloned();
                }
            }

            // --- Auto-load or command-line file ---
            let maybe_load = if env::args().count() > 1 {
                Some(env::args().nth(1).unwrap())
            } else {
                crate::asset::check_for_auto()
            };

            if let Some(s) = maybe_load {
                core.global.console = false;
                core.gui.disable_console();
                // Defer the actual load to the first frame (see `about_to_wait`).
                // It used to run right here, but unpacking a game and decoding its
                // oggs is heavy, and `resumed` is where the window is created — a
                // long block in it means the OS never gets to activate the window,
                // so it opened behind the terminal and never took keyboard focus.
                // (The load has to happen *somewhere* explicit: the original code
                // only stashed the arg in `pending_load` and relied on the boot
                // app's drop()->quit(), which never fires at cold boot, so a
                // `.game.png` arg silently never loaded at all.)
                self.deferred_load = Some(s);
            } else {
                #[cfg(feature = "include_auto")]
                {
                    core.global.console = false;
                    core.gui.disable_console();
                    let _id = core.bundle_manager.console_bundle_target;
                    // Load the *compiled-in* bundle on the first frame (same
                    // deferral as a CLI path, for the same window-activation
                    // reason). This branch used to only disable the console and
                    // leave loading to `check_for_auto`, which looks for
                    // auto.game.png *next to the executable* — meaningless inside
                    // an APK, where there's no exe directory to sit beside. So a
                    // bundled game never started on Android; on desktop it worked
                    // only because the file happened to be there too.
                    self.deferred_auto = true;
                }

                #[cfg(not(feature = "include_auto"))]
                {
                    #[cfg(not(feature = "studio"))]
                    core.gui.disable_console();
                }
            }
            println!("{} {}", "[ 1 ]".on_bright_purple(), "we built core".on_red());
            self.core = Some(core);
        }

        // --- Web: attach the canvas to the DOM, then build Core asynchronously.
        // wgpu adapter/device requests can't block the browser main thread, so
        // Core::new is awaited in a spawned task and installed by the frame loop
        // once ready. The Lua VM is NOT loaded here — it runs in a web worker
        // (wired separately); wgpu still initialises and clears the canvas. ---
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowExtWebSys;
            if let Some(canvas) = window.canvas() {
                attach_canvas_to_dom(&canvas);
                install_web_input_handlers(
                    &canvas,
                    self.pointer_lock_wanted.clone(),
                    #[cfg(feature = "audio")]
                    self.audio_ctx.clone(),
                );
            }
            let pending = self.pending_core.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let (core, catcher) = Core::new(window).await;
                ::log::info!("petrichor64: core ready — wgpu initialised");
                *pending.borrow_mut() = Some((core, catcher));
            });

            // Load the game bundle in parallel (fetch /game.game.png or embedded
            // default), unzip in memory; the frame loop routes it once ready.
            let pending_bundle = self.pending_bundle.clone();
            wasm_bindgen_futures::spawn_local(async move {
                const EMBEDDED: &[u8] = include_bytes!("../web/default.game.png");
                if let Some(bundle) = load_wasm_bundle(EMBEDDED).await {
                    *pending_bundle.borrow_mut() = Some(bundle);
                }
            });
        }
    }

    /// The native window is going away. On Android this fires whenever the app
    /// leaves the foreground — locking the screen is enough — and the surface built
    /// from that window dies with it. Stop drawing until `resumed` hands us a new
    /// one, rather than spending every frame failing to acquire a texture.
    ///
    /// Desktop and web never call this, so the flag simply stays false there.
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        ::log::info!("petrichor64: suspended — surface released");
        self.surface_lost = true;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        // Ignore events for unknown windows.
        if self.window.as_ref().map_or(true, |w| w.id() != id) {
            return;
        }

        match event {
            // ----------------------------------------------------------------
            WindowEvent::CloseRequested => {
                println!("Close requested");
                event_loop.exit();
            }

            // ----------------------------------------------------------------
            WindowEvent::Destroyed => {
                // Window gone — drop Core so GPU resources are freed.
                self.core = None;
                self.catcher = None;
            }

            // ----------------------------------------------------------------
            WindowEvent::Resized(physical_size) => {
                if let Some(core) = &mut self.core {
                    core.resize(physical_size);
                }
                // Keep the mouse-grab re-center position up to date.
                self.center = LogicalPosition::new(
                    physical_size.width as f64 / 2.0,
                    physical_size.height as f64 / 2.0,
                );
            }

            // ----------------------------------------------------------------
            WindowEvent::ScaleFactorChanged { .. } => {}

            // ----------------------------------------------------------------
            WindowEvent::RedrawRequested => {
                // Backgrounded on mobile: there is no surface to draw into, and
                // acquiring a texture would fail every frame until `resumed` builds
                // a new one.
                if self.surface_lost {
                    return;
                }
                if let Some(core) = &mut self.core {
                    match core.render() {
                        render::DrawState::Success => {}
                        // Surface lost / outdated — reconfigure and try next frame.
                        render::DrawState::Resize => {
                            let sz = core.gfx.size;
                            core.resize(sz);
                        }
                        // Timeout / skip — just wait for the next frame.
                        render::DrawState::Skip => {}
                    }
                }
            }

            // ----------------------------------------------------------------
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key,
                        logical_key,
                        state,
                        ..
                    },
                ..
            } => {
                // Update the bool key-state array for Lua.
                if let PhysicalKey::Code(keycode) = physical_key {
                    bit_check(&state, keycode, &mut self.bits);
                }

                // Feed typed characters directly into the console log so they
                // arrive on every key-repeat, not just once per Lua frame.
                if state == ElementState::Pressed {
                    if let Some(core) = &mut self.core {
                        if core.global.console {
                            match &logical_key {
                                Key::Character(c) => {
                                    // Filter out backtick (console toggle key).
                                    if c.as_str() != "`" {
                                        core.loggy.add(c.as_str());
                                    }
                                }
                                Key::Named(NamedKey::Space) => {
                                    core.loggy.add(" ");
                                }
                                Key::Named(NamedKey::Backspace) => {
                                    core.loggy.back();
                                }
                                _ => {}
                            }
                        }
                    }
                }
            }

            // ----------------------------------------------------------------
            WindowEvent::CursorMoved { position, .. } => {
                // Normalise against winit's window size — the same coordinate
                // space the cursor position is reported in. (gfx.size can lag on
                // the web, where it's clamped at init and only corrected on a
                // Resized event, giving mouse.x > 1.)
                let size = self.window.as_ref().map(|w| w.inner_size());
                if let (Some(core), Some(size)) = (&mut self.core, size) {
                    if size.width > 0 && size.height > 0 {
                        core.global.mouse_pos.x = position.x as f32 / size.width as f32;
                        core.global.mouse_pos.y = position.y as f32 / size.height as f32;
                    }
                }
            }

            // ----------------------------------------------------------------
            // Touch, folded into the mouse so every existing game and `mus()` call
            // works on a phone unchanged: the *primary* finger is the cursor, and
            // touching down is a left click.
            //
            // Primary means the first finger down that is still down — tracked by
            // winit's touch id, not "whichever event arrived". That distinction is
            // the whole point: a second finger landing and lifting during a drag
            // must not move the cursor or release the button, which is exactly what
            // id-less handling gets wrong. Extra fingers are ignored for now; when
            // multi-touch gestures arrive they belong in their own Lua command
            // rather than being smuggled through `mus`.
            //
            // Platform-independent on purpose: winit reports touch the same way on
            // Android, iOS and desktop touchscreens, so this also makes a Surface or
            // a touch-screen laptop work.
            WindowEvent::Touch(touch) => {
                use winit::event::TouchPhase;
                let size = self.window.as_ref().map(|w| w.inner_size());
                let (Some(core), Some(size)) = (&mut self.core, size) else {
                    return;
                };
                if size.width == 0 || size.height == 0 {
                    return;
                }
                match touch.phase {
                    TouchPhase::Started => {
                        if self.primary_touch.is_none() {
                            self.primary_touch = Some(touch.id);
                            // No delta on the first contact: there's no previous
                            // position to be relative to, and inventing one makes a
                            // tap look like a flick to anything reading `mus` delta.
                            self.touch_last = Some((touch.location.x, touch.location.y));
                            core.global.mouse_pos.x = touch.location.x as f32 / size.width as f32;
                            core.global.mouse_pos.y = touch.location.y as f32 / size.height as f32;
                            core.global.mouse_buttons[0] = 1.0;
                        }
                    }
                    TouchPhase::Moved => {
                        if self.primary_touch == Some(touch.id) {
                            core.global.mouse_pos.x = touch.location.x as f32 / size.width as f32;
                            core.global.mouse_pos.y = touch.location.y as f32 / size.height as f32;
                            // Accumulated, not assigned: several moves can land in
                            // one frame and a delta that overwrote its predecessor
                            // would under-report the drag. Pixels, to match the
                            // units DeviceEvent::MouseMotion reports on desktop.
                            if let Some((lx, ly)) = self.touch_last {
                                core.global.mouse_delta.x += (touch.location.x - lx) as f32;
                                core.global.mouse_delta.y += (touch.location.y - ly) as f32;
                            }
                            self.touch_last = Some((touch.location.x, touch.location.y));
                        }
                    }
                    TouchPhase::Ended | TouchPhase::Cancelled => {
                        if self.primary_touch == Some(touch.id) {
                            self.primary_touch = None;
                            self.touch_last = None;
                            core.global.mouse_buttons[0] = 0.0;
                            // Position deliberately left where the finger lifted,
                            // like a mouse that stopped moving. Games read the last
                            // position on release to decide what was hit.
                        }
                    }
                }
            }

            // ----------------------------------------------------------------
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(core) = &mut self.core {
                    let val = if state == ElementState::Pressed {
                        1.0f32
                    } else {
                        0.0
                    };
                    match button {
                        winit::event::MouseButton::Left => core.global.mouse_buttons[0] = val,
                        winit::event::MouseButton::Right => core.global.mouse_buttons[1] = val,
                        winit::event::MouseButton::Middle => core.global.mouse_buttons[2] = val,
                        winit::event::MouseButton::Forward => core.global.mouse_buttons[3] = val,
                        _ => {}
                    }
                }
            }

            // ----------------------------------------------------------------
            WindowEvent::MouseWheel { delta, .. } => {
                if let Some(core) = &mut self.core {
                    core.global.scroll_delta = match delta {
                        MouseScrollDelta::LineDelta(x, y) => (x, y),
                        MouseScrollDelta::PixelDelta(pos) => {
                            // Normalise pixel scroll to approximate line units.
                            (pos.x as f32 / 120.0, pos.y as f32 / 120.0)
                        }
                    };
                }
            }

            // ----------------------------------------------------------------
            // Authoritative modifier state. Tracking press/release by hand
            // leaves modifiers stuck when a keyup is lost — common on the web
            // when a Cmd/Ctrl combo triggers a browser action. The browser
            // reports the true modifier state here, so syncing from it self-heals
            // (a stuck Cmd would otherwise turn every Enter into Cmd+Enter →
            // fullscreen, etc.).
            WindowEvent::ModifiersChanged(mods) => {
                // Only the four collapsed modifier slots. Indexing by `KeyCode as
                // usize` (as this did) writes winit's *discriminant* into the
                // engine's own key-index space, where the numbers mean something
                // else entirely: ControlLeft is 55, which is f19 — and f19 is a
                // typeable character to `cin`, so merely holding Ctrl typed an "e".
                // Shift landed on f24, Cmd on f22. `bit_check` already collapses
                // left/right to these same slots, which is why modifiers otherwise
                // worked at all.
                let s = mods.state();
                let b = &mut self.bits.0;
                b[247] = s.alt_key();
                b[248] = s.control_key();
                b[249] = s.shift_key();
                b[250] = s.super_key();
            }

            // ----------------------------------------------------------------
            // Losing focus (tab-away, or a browser/OS shortcut stealing the
            // keyup) would otherwise leave keys stuck down. Clear all key state.
            WindowEvent::Focused(false) => {
                self.bits.0 = [false; 256];
            }

            // ----------------------------------------------------------------
            WindowEvent::DroppedFile(path) => {
                if let Some(core) = &mut self.core {
                    let s = path.as_os_str().to_string_lossy().to_string();
                    if core.global.boot_state {
                        core.global.pending_load = Some(s.clone());
                    }
                    core.bundle_manager.get_main_bundle().lua.call_drop(s);
                }
            }

            _ => {}
        }
    }

    /// Raw device events — used for relative mouse motion (independent of
    /// cursor position or acceleration).
    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta } = event {
            if let Some(core) = &mut self.core {
                core.global.mouse_delta = vec2(delta.0 as f32, delta.1 as f32);
            }
        }
    }

    /// Called once per iteration of the event loop before sleeping.
    /// This is where the 60 Hz Lua update runs.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        // Apply a command-line / auto-loaded game, now that the window exists and
        // the OS has had a chance to bring it to the front. Doing this inside
        // `resumed` blocked window activation long enough that the window opened
        // behind the terminal and never took keyboard focus.
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(path) = self.deferred_load.take() {
            if let Some(core) = self.core.as_mut() {
                crate::command::hard_reset(core);
                if let Err(e) = crate::command::load_app(core, Some(&path), None, None, None) {
                    core.loggy
                        .log(LogType::CoreError, &format!("failed to load {}: {}", path, e));
                }
            }
        }

        // Bring up a `--overlay` tool, once the app it edits is in place.
        #[cfg(not(target_arch = "wasm32"))]
        if self.deferred_load.is_none() {
            if let Some(path) = self.deferred_overlay.take() {
                if let Some(core) = self.core.as_mut() {
                    match crate::command::load_overlay(core, &path) {
                        Ok(id) => ::log::info!("overlay '{}' up as bundle {}", path, id),
                        Err(e) => ::log::error!("overlay '{}' failed: {}", path, e),
                    }
                }
            }
        }

        // The game baked in at compile time (`include_auto`), for builds that have
        // no filesystem to find it on — an APK, or a relocated desktop binary.
        #[cfg(all(not(target_arch = "wasm32"), feature = "include_auto"))]
        if self.deferred_auto {
            self.deferred_auto = false;
            if let Some(core) = self.core.as_mut() {
                crate::command::hard_reset(core);
                let payload = include_bytes!("../auto.game.png").to_vec();
                ::log::info!("loading included game ({} bytes)", payload.len());
                if let Err(e) =
                    crate::command::load_app(core, Some("INCLUDE_AUTO"), Some(payload), None, None)
                {
                    core.loggy.log(
                        LogType::CoreError,
                        &format!("failed to load the included game: {}", e),
                    );
                }
            }
        }

        // Install the asynchronously-built Core once it's ready (web only).
        #[cfg(target_arch = "wasm32")]
        if self.core.is_none() {
            if let Some((core, catcher)) = self.pending_core.borrow_mut().take() {
                // Share the AudioContext with the gesture handlers so they can
                // resume it on first interaction (browsers start it suspended).
                #[cfg(feature = "audio")]
                {
                    *self.audio_ctx.borrow_mut() =
                        core.web_audio.as_ref().map(|w| w.context());
                }
                self.core = Some(core);
                self.catcher = Some(catcher);
            }
        }

        // Drive the VM web worker (§4d). Spawn it once; once its wasm is ready,
        // send Init + an initial Load; then post a Loop each iteration and drain
        // whatever VmToHost it produced. For now the drained messages are just
        // logged — applying them to the renderer is the next step.
        #[cfg(target_arch = "wasm32")]
        {
            use crate::worker_protocol::HostToVm;
            if self.worker.is_none() {
                match crate::web_worker::WorkerHandle::spawn("/worker.js") {
                    Ok(w) => {
                        ::log::info!("petrichor64: VM worker spawned");
                        self.worker = Some(w);
                    }
                    Err(e) => web_sys::console::error_1(
                        &format!("VM worker spawn failed: {:?}", e).into(),
                    ),
                }
            }
            // Settle how the worker's sound commands travel, once. Either it gets
            // the worklet's command lane (a note goes straight from Lua to the audio
            // thread) or it's told none is coming, so it stops holding commands back
            // and routes them here for the fallback scheduler. Until one of the two
            // arrives the worker buffers — see web/worker.js for why switching
            // mid-stream would reorder `instr`/`smpl` against the notes using them.
            #[cfg(feature = "audio")]
            if !self.sound_lane_settled {
                if let (Some(w), Some(core)) = (&self.worker, self.core.as_mut()) {
                    if w.is_ready() {
                        if let Some(out) = core.web_audio.as_mut() {
                            if let Some(port) = out.take_command_port() {
                                w.give_sound_port(port);
                                self.sound_lane_settled = true;
                                ::log::info!(
                                    "web audio: VM worker owns the command lane (notes bypass \
                                     the main thread)"
                                );
                            } else if out.is_fallback() {
                                w.tell_no_sound_lane();
                                self.sound_lane_settled = true;
                            }
                        }
                    }
                }
            }

            let mut drained: Vec<crate::worker_protocol::VmToHost> = Vec::new();
            if let Some(w) = &self.worker {
                if w.is_ready() {
                    if !self.worker_inited {
                        // Once the bundle has finished unzipping, route it: load
                        // textures into the GPU atlas + decode sounds into the
                        // mixer (main thread), and send scripts + main() to the
                        // VM worker. Until then, retry next frame.
                        let bundle = self.pending_bundle.borrow_mut().take();
                        if let Some(bundle) = bundle {
                            if let Some(core) = self.core.as_mut() {
                                // Set up bundle 0's main-side world (GPU meshing +
                                // mapper); the worker owns the tile data and syncs
                                // chunks here.
                                core.world.init_local(0);
                                for (name, png) in &bundle.textures {
                                    core.load_wasm_texture(name, png);
                                }
                                #[cfg(feature = "audio")]
                                crate::asset::load_sounds_from_buffers(
                                    bundle.sounds.clone(),
                                    &core.singer,
                                    &mut core.loggy,
                                );
                            }
                            w.post(&HostToVm::Init {
                                bundle_id: 0,
                                width: 256,
                                height: 256,
                            });
                            for (name, content) in &bundle.scripts {
                                w.post(&HostToVm::Load {
                                    name: name.clone(),
                                    content: content.clone(),
                                });
                            }
                            w.post(&HostToVm::Main);
                            self.worker_inited = true;
                        }
                    }
                    // Console toggle (backtick), submit/history, and system
                    // shortcuts run on the main thread, same as native.
                    if let Some(core) = self.core.as_mut() {
                        controls::controls_evaluate(
                            core,
                            event_loop,
                            &self.bits,
                            &self.bits_prev,
                        );
                    }
                    // Same as native: re-trace the cursor ray against this frame's
                    // pointer position before the VM sees it, since render runs later.
                    if let Some(core) = self.core.as_mut() {
                        core.refresh_cursor_ray();
                    }
                    // Keys are already in self.bits.0 (window_event's bit_check);
                    // copy the mouse/analog state from core.global into bits.1,
                    // mirroring the native about_to_wait input copy.
                    if let Some(core) = self.core.as_ref() {
                        let g = &core.global;
                        self.bits.1[0] = g.mouse_pos.x;
                        self.bits.1[1] = g.mouse_pos.y;
                        self.bits.1[2] = g.mouse_delta.x;
                        self.bits.1[3] = g.mouse_delta.y;
                        self.bits.1[4] = g.mouse_buttons[0];
                        self.bits.1[5] = g.mouse_buttons[1];
                        self.bits.1[6] = g.mouse_buttons[2];
                        self.bits.1[7] = g.scroll_delta.0;
                        self.bits.1[8] = g.cursor_projected_pos.x;
                        self.bits.1[9] = g.cursor_projected_pos.y;
                        self.bits.1[10] = g.cursor_projected_pos.z;

                        // Mirror the app's grab intent into the pointer-lock
                        // flag the canvas click handler reads. Release the lock
                        // immediately when the app drops grab or the console opens.
                        let want = g.mouse_grab && !g.console;
                        *self.pointer_lock_wanted.borrow_mut() = want;
                        if !want {
                            if let Some(doc) =
                                web_sys::window().and_then(|w| w.document())
                            {
                                if doc.pointer_lock_element().is_some() {
                                    doc.exit_pointer_lock();
                                }
                            }
                        }
                    }
                    // Only drive the loop once the VM has been Init'd + Loaded
                    // (bundle routed). Posting a Loop before Init — e.g. while the
                    // bundle is still unzipping — errors "message before Init".
                    if self.worker_inited {
                        // With the console open, the app must not receive input —
                        // the keys go to the console. Send a neutral snapshot.
                        let console_open =
                            self.core.as_ref().map_or(false, |c| c.global.console);
                        if console_open {
                            w.post(&HostToVm::Loop {
                                keys: vec![0u8; 256],
                                analog: vec![0f32; 11],
                            });
                        } else {
                            w.post(&HostToVm::loop_from(&self.bits));
                        }
                        drained = w.drain();
                    }
                }
            }
            for m in drained {
                if let Some(core) = self.core.as_mut() {
                    core.apply_vm_message(m);
                }
            }

            // Keep the web audio queue filled ahead of the audio clock (glitch-free
            // through render jank). Runs every frame once Core is up.
            //
            // *After* the worker's messages are applied, deliberately. The VM's
            // notes arrive as `VmToHost::Sound` in the loop above and land in the
            // command channel; pumping before that meant every note sat in the
            // channel until the *next* frame — a full frame of latency (~16ms at
            // 60fps) handed away for free, which is a lot next to the worklet's
            // 2.7ms render quantum. Costs the fallback scheduler nothing: it
            // schedules ~90ms ahead regardless of where in the frame it runs.
            #[cfg(feature = "audio")]
            if let Some(core) = self.core.as_mut() {
                if let Some(w) = core.web_audio.as_mut() {
                    w.pump();
                }
            }
            // Consume per-frame input deltas so they don't persist to next frame.
            if let Some(core) = self.core.as_mut() {
                core.global.mouse_delta = vec2(0., 0.);
                core.global.scroll_delta = (0., 0.);
            }
            self.bits_prev = self.bits.0;
        }

        let now = Instant::now();

        if now >= self.next_frame_time {
            let core = match self.core.as_mut() {
                Some(c) => c,
                None => return,
            };
            let catcher = match self.catcher.as_ref() {
                Some(c) => c,
                None => return,
            };

            #[cfg(not(target_arch = "wasm32"))]
            core.loop_helper.loop_start();

            // Grab a clone of the window Arc so we can pass it to
            // state_change_checker without holding a borrow of self.window
            // while also mutably borrowing self.core.
            let win_arc = match self.window.clone() {
                Some(w) => w,
                None => return,
            };

            state_change_checker(core, event_loop, &win_arc, self.center);

            // Release grabbed mouse if the console was just opened.
            if core.global.console && core.global.mouse_grabbed_state {
                win_arc.set_cursor_visible(true);
                let _ = win_arc.set_cursor_grab(CursorGrabMode::None);
                core.global.mouse_grabbed_state = false;
            }

            // Process incoming MainCommands from Lua threads. When a Lua loop
            // completes, update() returns the freshly-built entity instance
            // buffers; inject them into the scene the renderer draws. (These were
            // previously discarded, so no entity/plane ever reached the 3D pass.)
            if let Some(ib) = core.update(catcher) {
                core.instance_buffers = ib;
            }

            // Bring the unprojected cursor up to date with this frame's pointer
            // position before handing it to Lua — render, which is where the ray is
            // otherwise computed, doesn't run until after the Lua loop.
            core.refresh_cursor_ray();

            // Copy mouse / analogue state into the float portion of bits
            // so Lua can read cursor and scroll data.
            self.bits.1[0] = core.global.mouse_pos.x;
            self.bits.1[1] = core.global.mouse_pos.y;
            self.bits.1[2] = core.global.mouse_delta.x;
            self.bits.1[3] = core.global.mouse_delta.y;
            self.bits.1[4] = core.global.mouse_buttons[0];
            self.bits.1[5] = core.global.mouse_buttons[1];
            self.bits.1[6] = core.global.mouse_buttons[2];
            self.bits.1[7] = core.global.scroll_delta.0;
            self.bits.1[8] = core.global.cursor_projected_pos.x;
            self.bits.1[9] = core.global.cursor_projected_pos.y;
            self.bits.1[10] = core.global.cursor_projected_pos.z;

            // Evaluate system shortcuts and console key commands.
            controls::controls_evaluate(core, event_loop, &self.bits, &self.bits_prev);

            // Reset per-frame deltas after they've been consumed.
            core.global.mouse_delta = vec2(0., 0.);
            core.global.scroll_delta = (0., 0.);

            // Send the Lua loop message (~60 Hz).
            //
            // Nothing gets input while the console is open — the keys are being typed
            // into the console, and they must not also drive the game. The wasm path
            // already did this; native didn't, so on desktop typing a command was
            // simultaneously playing the game. Same rule as an overlay taking input,
            // just with the console as the surface on top.
            let console_open = core.global.console;
            let quiet = ControlState::default();
            let feed = if console_open { &quiet } else { &self.bits };
            core.bundle_manager
                .call_loop(&mut core.completed_bundles, feed);

            // Save current key state for next frame's pressed/released detection.
            self.bits_prev = self.bits.0;

            // Request a GPU frame; rendering happens in RedrawRequested.
            win_arc.request_redraw();

            // Advance to the next target frame time (avoids drift).
            self.next_frame_time += self.frame_duration;
        }

        // Sleep until the next frame is due.
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_frame_time));
    }
}

/// CLI dispatch, run before launching the engine. Currently handles the
/// bundler: `Petrichor64 pack <dir> [out]` (alias `bundle`) packs a folder into
/// a `.game.png` without opening a window. Returns true if a CLI command was
/// handled, so `main()` should exit instead of starting the engine.
#[cfg(all(feature = "headed", not(target_arch = "wasm32")))]
pub fn run_cli() -> bool {
    let args: Vec<String> = std::env::args().collect();
    if args.len() >= 2 && (args[1] == "pack" || args[1] == "bundle") {
        let dir = args.get(2).map(|s| s.as_str()).unwrap_or(".");
        let out = args.get(3).map(|s| s.as_str());
        let mut loggy = crate::log::Loggy::new();
        // pack_zip uses tokio::fs, so it needs a Tokio runtime (not pollster).
        let rt = tokio::runtime::Runtime::new().expect("failed to start tokio runtime");
        match rt.block_on(crate::asset::pack_folder(dir, out, &mut loggy)) {
            Ok(()) => println!("packed '{}' -> {}", dir, out.unwrap_or("<dir>.game.png")),
            Err(e) => {
                eprintln!("pack failed: {}", e);
                std::process::exit(1);
            }
        }
        return true;
    }
    false
}

/// Android's entry point. There is no `main` on Android: the activity loads this
/// shared library and calls `android_main`, handing over the `AndroidApp` that owns
/// the native window and the event queue — which is why the event loop has to be
/// built from it rather than from scratch.
///
/// `no_mangle` because the activity looks the symbol up by name, and the crate is
/// already built as a `cdylib` (for wasm), which is exactly what an APK needs.
///
/// **Not yet complete** — two pieces stand between this and running a game on a
/// device, both noted in PLAN.md:
///  - *Where the game comes from.* Desktop takes a path, web fetches or falls back
///    to an embedded bundle; an APK has neither. The game wants to be read out of
///    the APK's assets via `AndroidApp::asset_manager()`.
///  - *Surface lifecycle.* Android destroys the native window when the app is
///    backgrounded and `resumed` fires again on return. `resumed` here only builds a
///    window when there isn't one, so the wgpu surface would be stale — the surface
///    needs recreating without rebuilding `Core` and losing the running game.
#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn android_main(app: android_activity::AndroidApp) {
    use winit::platform::android::EventLoopBuilderExtAndroid;

    android_logger::init_once(
        android_logger::Config::default().with_max_level(::log::LevelFilter::Info),
    );
    ::log::info!("petrichor64: android_main");

    let event_loop = match EventLoop::<()>::builder().with_android_app(app).build() {
        Ok(el) => el,
        Err(e) => {
            // No dialog and no stdout here; logcat is the only channel.
            ::log::error!("petrichor64: event loop build failed: {}", e);
            return;
        }
    };

    let mut engine = App::default();
    if let Err(e) = event_loop.run_app(&mut engine) {
        ::log::error!("petrichor64: event loop exited with {:?}", e);
    }
}

#[cfg(all(feature = "headed", not(target_arch = "wasm32")))]
pub fn start() {
    env_logger::init();

    let event_loop = {
        #[allow(unused_mut)]
        let mut builder = EventLoop::<()>::builder();
        // macOS: a bare binary launched from a terminal is not automatically the
        // *active application*. Its window can appear (even in front) while the
        // app itself never activates, so key events keep going to whatever was
        // focused before — the terminal — and the engine looks like it ignores all
        // input. Nothing set a policy before, which left this to luck: anything
        // that shifted startup timing (which boot app loads, whether the console is
        // disabled) changed whether we happened to win activation.
        //
        // `Regular` makes us an ordinary foreground app (dock icon, menu bar), and
        // activating over other apps takes focus the way a double-clicked .app does.
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::{ActivationPolicy, EventLoopBuilderExtMacOS};
            builder.with_activation_policy(ActivationPolicy::Regular);
            builder.with_activate_ignoring_other_apps(true);
        }
        match builder.build() {
            Ok(el) => el,
            Err(e) => {
                error_window(Box::new(e));
                return;
            }
        }
    };

    let mut app = App::default();
    // run_app drives the event loop; App::resumed() does all the setup.
    if let Err(e) = event_loop.run_app(&mut app) {
        match e {
            EventLoopError::ExitFailure(e) => {
                eprintln!(" exit error code {e}")
            }
            _ => eprintln!("unknown window exit error"),
        }
    };
}

/// Web entry point, invoked from `main()` (wasm-bindgen wraps the bin's `main`
/// as the module entry). Unlike native `start()`, the browser event loop must
/// not block: winit's `spawn_app` hands control back to JS and drives frames
/// via requestAnimationFrame. App::resumed then initialises the engine
/// asynchronously (wgpu adapter/device requests can't be blocked on the web).
/// A game bundle unpacked in memory for the web build. No filesystem: scripts go
/// to the VM worker, textures to the GPU atlas, sounds are decoded into the mixer.
#[cfg(target_arch = "wasm32")]
struct WasmBundle {
    /// (stem, lua source)
    scripts: Vec<(String, String)>,
    /// (stem, png bytes)
    textures: Vec<(String, Vec<u8>)>,
    /// (path, ogg bytes) — passed as-is to load_sounds_from_buffers.
    sounds: Vec<(String, Vec<u8>)>,
}

/// Fetch a URL's bytes, or None on any failure (missing file, network error).
#[cfg(target_arch = "wasm32")]
async fn fetch_bytes(url: &str) -> Option<Vec<u8>> {
    use wasm_bindgen::JsCast;
    let win = web_sys::window()?;
    let resp_val = wasm_bindgen_futures::JsFuture::from(win.fetch_with_str(url))
        .await
        .ok()?;
    let resp: web_sys::Response = resp_val.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let buf = wasm_bindgen_futures::JsFuture::from(resp.array_buffer().ok()?)
        .await
        .ok()?;
    Some(js_sys::Uint8Array::new(&buf).to_vec())
}

/// Load the web game bundle: prefer a deployed `/game.game.png` (fetched over
/// HTTP — swap games without a rebuild), else the embedded default baked in at
/// build time. Unzips the `.game.png` in memory (same as the native packed path)
/// and splits it into scripts/textures/sounds for the caller to route.
#[cfg(target_arch = "wasm32")]
async fn load_wasm_bundle(embedded: &'static [u8]) -> Option<WasmBundle> {
    // A bundle is a raw zip (`PK\x03\x04`) or a PNG-prefixed .game.png. A dev
    // server (trunk) answers a missing /game.game.png with its index.html SPA
    // fallback (200 OK), so verify the bytes actually look like a bundle —
    // otherwise we'd try to unzip an HTML page. Anything else → embedded default.
    let looks_like_bundle = |b: &[u8]| {
        b.len() > 8
            && (b[..4] == [0x50, 0x4B, 0x03, 0x04] // raw zip
                || b[..8] == [0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]) // PNG
    };
    let bytes = match fetch_bytes("/game.game.png").await {
        Some(b) if looks_like_bundle(&b) => {
            ::log::info!("web: loaded /game.game.png ({} bytes)", b.len());
            b
        }
        _ => {
            ::log::info!("web: no deployed /game.game.png, using embedded default bundle");
            embedded.to_vec()
        }
    };

    let mut loggy = crate::log::Loggy::new();
    let mut archive = crate::file_util::get_archive(bytes, &mut loggy).await.ok()?;
    let map = crate::file_util::unpack_and_walk(
        &mut archive,
        vec!["assets", "scripts", "sounds"],
        &mut loggy,
    )
    .await
    .ok()?;

    let stem = |n: &str| {
        std::path::Path::new(n)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or(n)
            .to_string()
    };
    let mut scripts = Vec::new();
    if let Some(v) = map.get("scripts") {
        for (name, bytes) in v {
            if name.ends_with(".lua") && !name.ends_with("ignore.lua") {
                scripts.push((stem(name), String::from_utf8_lossy(bytes).to_string()));
            }
        }
    }
    let textures = map
        .get("assets")
        .map(|v| {
            v.iter()
                .filter(|(n, _)| n.ends_with(".png"))
                .map(|(n, b)| (stem(n), b.clone()))
                .collect()
        })
        .unwrap_or_default();
    let sounds = map
        .get("sounds")
        .map(|v| {
            v.iter()
                .filter(|(n, _)| n.ends_with(".ogg"))
                .map(|(n, b)| (n.clone(), b.clone()))
                .collect()
        })
        .unwrap_or_default();
    Some(WasmBundle {
        scripts,
        textures,
        sounds,
    })
}

#[cfg(all(feature = "headed", target_arch = "wasm32"))]
pub fn start() {
    use winit::platform::web::EventLoopExtWebSys;

    // The bin's main() runs on module init in EVERY wasm instance, including the
    // VM web worker's. A worker has no `Window` (only WorkerGlobalScope), so
    // winit's event loop would panic there ("only callable from inside the
    // Window"). The worker drives itself via worker_init/worker_receive, so
    // start() must no-op when there's no Window.
    if web_sys::window().is_none() {
        return;
    }

    console_error_panic_hook::set_once();
    // Route `log` output to the browser console.
    let _ = console_log::init_with_level(::log::Level::Info);

    let event_loop = match EventLoop::<()>::new() {
        Ok(el) => el,
        Err(e) => {
            web_sys::console::error_1(&format!("event loop error: {e}").into());
            return;
        }
    };

    // spawn_app returns immediately; the closure-owned App lives on inside the
    // browser's event loop.
    event_loop.spawn_app(App::default());
}

/// Append winit's canvas to the page. Prefers an element with id
/// `petrichor64-root` (provided by the `<petrichor-64>` web component) so the
/// engine renders inside the component; falls back to <body>.
#[cfg(all(feature = "headed", target_arch = "wasm32"))]
fn attach_canvas_to_dom(canvas: &web_sys::HtmlCanvasElement) {
    use wasm_bindgen::JsCast;
    let document = match web_sys::window().and_then(|w| w.document()) {
        Some(d) => d,
        None => return,
    };
    // Give the canvas real pixels up front so it's visible and the first
    // Resized event reports a nonzero size (winit will keep it in sync after).
    if canvas.width() == 0 {
        canvas.set_width(640);
    }
    if canvas.height() == 0 {
        canvas.set_height(548);
    }
    // Pin the CSS display size. Without it the canvas displays at its backing
    // buffer's pixel size, so winit's DPR-scaled buffer grows the client rect,
    // which grows the buffer again — a runaway that eventually exceeds the GPU's
    // max texture size and aborts the module. A fixed CSS size decouples display
    // from buffer; a host page can still override via CSS on #petrichor64-root.
    let style = canvas.style();
    let _ = style.set_property("width", "640px");
    let _ = style.set_property("height", "548px");
    let parent = document
        .get_element_by_id("petrichor64-root")
        .or_else(|| document.body().map(|b| b.unchecked_into::<web_sys::Element>()));
    if let Some(parent) = parent {
        let _ = parent.append_child(canvas);
    }
}

/// Wire the web-only input niceties onto the canvas:
/// - click captures the mouse (pointer-lock) when the app has asked for grab
///   (`pointer_lock_wanted`); browsers only grant lock from a user gesture, so
///   it can't be done from the frame loop.
/// - while locked, reload shortcuts (Cmd/Ctrl+R, F5) are swallowed so the
///   embedded game doesn't lose the tab. Cmd/Ctrl+W/T/Q are OS-reserved and
///   cannot be intercepted by a page.
///
/// Both closures are `forget()`-leaked deliberately: they must live for the
/// whole page session, which matches the canvas lifetime.
#[cfg(all(feature = "headed", target_arch = "wasm32"))]
fn install_web_input_handlers(
    canvas: &web_sys::HtmlCanvasElement,
    pointer_lock_wanted: std::rc::Rc<std::cell::RefCell<bool>>,
    #[cfg(feature = "audio")] audio_ctx: std::rc::Rc<
        std::cell::RefCell<Option<web_sys::AudioContext>>,
    >,
) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

    // Resume the AudioContext on the first real user gesture. Browsers start a
    // context created without a gesture (our boot-time init) suspended, and only
    // honor resume() from inside a gesture handler — so do it here, on
    // pointerdown and keydown (capture phase). Idempotent: resuming a running
    // context is a no-op, so firing every gesture is fine.
    #[cfg(feature = "audio")]
    if let Some(win) = web_sys::window() {
        for evt in ["pointerdown", "keydown"] {
            let ctx = audio_ctx.clone();
            let on_gesture = Closure::<dyn FnMut()>::new(move || {
                if let Some(c) = ctx.borrow().as_ref() {
                    let _ = c.resume();
                }
            });
            let _ = win.add_event_listener_with_callback_and_bool(
                evt,
                on_gesture.as_ref().unchecked_ref(),
                true,
            );
            on_gesture.forget();
        }
    }

    let lock_canvas = canvas.clone();
    let pointerdown =
        Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |_e: web_sys::MouseEvent| {
            if *pointer_lock_wanted.borrow() {
                lock_canvas.request_pointer_lock();
            }
        });
    // Use `pointerdown` + capture phase (true): winit drives input through the
    // Pointer Events API and preventDefault's it, which SUPPRESSES the legacy
    // `mousedown`/`click` compatibility events — so a `mousedown` listener never
    // fires. `pointermove` still fires (that's why the camera pans). Capturing
    // runs before winit's handler and still counts as the pointer-lock gesture.
    let _ = canvas.add_event_listener_with_callback_and_bool(
        "pointerdown",
        pointerdown.as_ref().unchecked_ref(),
        true,
    );
    pointerdown.forget();

    // Surface a rejected lock — the usual causes are an embedding iframe missing
    // allow="pointer-lock" or the browser's brief post-Esc cooldown.
    if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
        let err = Closure::<dyn FnMut()>::new(|| {
            web_sys::console::warn_1(
                &"petrichor64: pointer-lock request was rejected (iframe allow=\"pointer-lock\"?)"
                    .into(),
            );
        });
        let _ = doc
            .add_event_listener_with_callback("pointerlockerror", err.as_ref().unchecked_ref());
        err.forget();
    }

    if let Some(win) = web_sys::window() {
        let keydown = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(
            move |e: web_sys::KeyboardEvent| {
                let locked = web_sys::window()
                    .and_then(|w| w.document())
                    .and_then(|d| d.pointer_lock_element())
                    .is_some();
                if !locked {
                    return;
                }
                let k = e.key();
                let mod_key = e.meta_key() || e.ctrl_key();
                if k == "F5" || (mod_key && (k == "r" || k == "R")) {
                    e.prevent_default();
                }
            },
        );
        // Capture phase so we run before winit's key handler (which stops
        // propagation on the canvas) and can preventDefault the reload.
        let _ = win.add_event_listener_with_callback_and_bool(
            "keydown",
            keydown.as_ref().unchecked_ref(),
            true,
        );
        keydown.forget();
    }
}

/// Terminal entry point: no window/GPU, but draws via the software rasterizer
/// in `crate::tui` instead of running fully blind. See tui/mod.rs.
#[cfg(all(not(feature = "headed"), feature = "render-tui"))]
pub fn start() {
    crate::tui::start();
}

/// Headless entry point: no window/GPU/render at all. Builds the core, loads
/// the default app, then drives the 60Hz lua loop while feeding stdin lines in
/// as console commands.
#[cfg(all(not(feature = "headed"), not(feature = "render-tui")))]
pub fn start() {
    env_logger::init();
    let (mut core, catcher) = block_on(Core::new());

    // Boot to the built-in "nil" app (embedded nil.game.png). A game can be
    // loaded afterward via the CLI/console; nil is the no-app fallback.
    crate::command::load_empty(&mut core);

    let frame = Duration::from_secs_f32(1.0 / FPS);
    let bits = ControlState::default();
    loop {
        let start = Instant::now();

        // Feed any queued stdin lines in as console commands.
        while let Ok(line) = core.cli_thread_receiver.try_recv() {
            core_console_command(&mut core, line.trim());
        }

        core.update(&catcher);
        core.bundle_manager
            .call_loop(&mut core.completed_bundles, &bits);

        if let Some(rem) = frame.checked_sub(start.elapsed()) {
            std::thread::sleep(rem);
        }
    }
}

pub fn core_console_command(core: &mut Core, com_in: &str) {
    let mut com = com_in.trim().to_owned();
    if let Some(alias) = core.global.aliases.get(&com) {
        com = alias.to_string();
    }
    for c in com.split("&&") {
        match crate::command::run_con_sys(core, c) {
            Ok(false) => {
                // A non-system command routes to the running game's Lua VM. With
                // no game loaded, get_lua() would panic ("No bundles loaded!") —
                // on the web that surfaces as a raw devtools error. Fail softly
                // into the engine console instead.
                if !core.bundle_manager.has_bundles() {
                    core.loggy.log(
                        LogType::LuaError,
                        &format!("no game loaded — '{}' has nowhere to run", c),
                    );
                    continue;
                }
                let mut ltype = LogType::Lua;
                let r = match core.bundle_manager.get_lua().func(c) {
                    Ok(v) => match v {
                        LuaResponse::String(s) => Some(s),
                        LuaResponse::Number(n) => Some(n.to_string()),
                        LuaResponse::Integer(i) => Some(i.to_string()),
                        LuaResponse::Bool(b) => Some(b.to_string()),
                        LuaResponse::Table(t) => {
                            let mut s = String::new();
                            s.push('{');
                            for (k, v) in t {
                                s.push_str(&format!("{}: {}, ", k, v));
                            }
                            s.push('}');
                            Some(s)
                        }
                        _ => None,
                    },
                    Err(e) => {
                        ltype = LogType::LuaError;
                        Some(e.to_string())
                    }
                };
                if let Some(result) = r {
                    core.loggy.log(ltype, &result);
                }
            }
            Ok(true) => {}
            Err(e) => {
                core.loggy.log(LogType::LuaError, &format!("!!{}", e));
            }
        }
    }
}

impl Core {
    /// Apply one `VmToHost` message from the web worker to the main-thread engine
    /// (wasm). The worker owns the VM; these messages are how its effects reach
    /// wgpu. Spawn builds a render entity from the delivered `LuaEnt` mirror; Cam
    /// moves the camera. SetImg/Globals are wired next.
    /// Decode a PNG asset and register it in the texture atlas (wasm). Used to
    /// load a bundle's loaded assets (e.g. example.png) so the worker's
    /// make('example') resolves a real texture.
    #[cfg(all(feature = "headed", target_arch = "wasm32"))]
    fn load_wasm_texture(&mut self, name: &str, png: &[u8]) {
        match image::load_from_memory(png) {
            Ok(dyn_img) => {
                self.tex_manager.overwrite_texture(
                    name,
                    dyn_img.to_rgba8(),
                    &mut self.world,
                    0,
                    &mut self.loggy,
                );
                self.tex_manager
                    .refinalize(&self.gfx.queue, &self.gfx.master_texture);
            }
            Err(e) => web_sys::console::error_1(
                &format!("failed to decode asset '{}': {}", name, e).into(),
            ),
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn apply_vm_message(&mut self, msg: crate::worker_protocol::VmToHost) {
        use crate::worker_protocol::VmToHost;
        match msg {
            VmToHost::Spawn(lent) => {
                #[cfg(feature = "headed")]
                self.ent_manager
                    .create_from_lua_ent(&self.tex_manager, &self.model_manager, lent);
            }
            VmToHost::Cam { pos, rot } => {
                if let Some(p) = pos {
                    self.global.cam_pos = glam::vec3(p[0], p[1], p[2]);
                }
                if let Some(r) = rot {
                    self.global.simple_cam_rot = glam::vec2(r[0], r[1]);
                }
            }
            VmToHost::Light {
                dir,
                color,
                ambient,
                sky,
                ground,
            } => {
                if let Some(d) = dir {
                    self.global.light_dir = glam::vec3(d[0], d[1], d[2]);
                }
                if let Some(c) = color {
                    self.global.light_color = glam::vec3(c[0], c[1], c[2]);
                }
                if let Some(a) = ambient {
                    self.global.light_ambient = a;
                }
                if sky.is_some() || ground.is_some() {
                    let s = sky.or(ground).unwrap_or([0., 0., 0.]);
                    let g = ground.or(sky).unwrap_or([0., 0., 0.]);
                    self.global.amb_sky = glam::vec4(s[0], s[1], s[2], 1.);
                    self.global.amb_ground = glam::vec4(g[0], g[1], g[2], 0.);
                }
            }
            VmToHost::Fog(a) => {
                self.global.fog_color = glam::vec4(a[0], a[1], a[2], a[3]);
            }
            VmToHost::MouseGrab(on) => {
                // Desired grab state; the frame loop mirrors it into the
                // pointer-lock intent (actual lock waits for a canvas click).
                self.global.mouse_grab = on;
            }
            VmToHost::SetImg { name, w, h, px } => {
                #[cfg(feature = "headed")]
                if let Some(img) = image::RgbaImage::from_raw(w, h, px) {
                    // Registers a new texture (or overlays an existing one) in the
                    // atlas, then re-uploads it to the GPU.
                    self.tex_manager
                        .overwrite_texture(&name, img, &mut self.world, 0, &mut self.loggy);
                    self.tex_manager
                        .refinalize(&self.gfx.queue, &self.gfx.master_texture);
                }
            }
            VmToHost::EntUpdate(xforms) => {
                // Refresh the main-thread entity mirrors with the worker's live
                // transforms. check_ents reads these each frame (via build_meta),
                // so updating them here moves the rendered entity.
                #[cfg(feature = "headed")]
                for xf in xforms {
                    for (eref, _ent, _uni) in self.ent_manager.ent_array.iter_mut() {
                        let mut matched = false;
                        let _ = eref.with_mut(|l| {
                            if l.get_id() == xf.id {
                                l.x = xf.x as f64;
                                l.y = xf.y as f64;
                                l.z = xf.z as f64;
                                l.rot_x = xf.rx as f64;
                                l.rot_y = xf.ry as f64;
                                l.rot_z = xf.rz as f64;
                                l.scale = xf.scale as f64;
                                matched = true;
                            }
                            Ok(())
                        });
                        if matched {
                            break;
                        }
                    }
                }
            }
            VmToHost::EntRemove(ids) => {
                // Drop the render mirrors of entities that died in the VM.
                #[cfg(feature = "headed")]
                {
                    let before = self.ent_manager.ent_array.len();
                    self.ent_manager.ent_array.retain(|(eref, _ent, _uni)| {
                        let mut keep = true;
                        let _ = eref.with_ref(|l| {
                            if ids.contains(&l.get_id()) {
                                keep = false;
                            }
                            Ok(())
                        });
                        keep
                    });
                    // The drawn instances come from render_hash, not ent_array
                    // directly — mark it dirty so check_ents rebuilds it without
                    // the removed entities on the next loop.
                    if self.ent_manager.ent_array.len() != before {
                        self.ent_manager.hash_dirty = true;
                    }
                }
            }
            VmToHost::WorldSync { chunks, dropped } => {
                // Worker sent dirty tile chunks; mesh them into GPU chunk models.
                #[cfg(feature = "headed")]
                {
                    let chunks: Vec<crate::tile::Chunk> =
                        chunks.into_iter().map(|c| c.into_chunk()).collect();
                    self.world.process_sync(
                        &self.gfx.device,
                        0,
                        chunks,
                        dropped,
                        &self.model_manager,
                    );
                }
            }
            VmToHost::MapTex { name, index } => {
                // Resolve the tile-texture name to its atlas uv and record it so
                // process_sync can look up chunk cells by their int index.
                #[cfg(feature = "headed")]
                {
                    let uv = self.tex_manager.get_tex(&name);
                    self.world.set_local_tex(0, index, uv);
                }
            }
            VmToHost::Globals(_) => {
                // TODO: Globals → screen effects.
            }
            VmToHost::LoopComplete { .. } => {
                // Native rebuilds entity instance buffers when a LoopComplete
                // arrives via the mpsc catcher; on wasm that signal comes through
                // here instead, so rebuild them now or nothing 3D ever draws.
                #[cfg(feature = "headed")]
                {
                    self.instance_buffers = self.ent_manager.check_ents(
                        &self.gfx.device,
                        &self.tex_manager,
                        &self.model_manager,
                        self.global.iteration,
                    );
                }
            }
            VmToHost::Error(s) => {
                web_sys::console::error_1(&format!("[vm error] {}", s).into());
            }
            // Replay a VM sound command into the main thread's cpal stream (the
            // worker has no audio device, so it forwarded it here).
            #[cfg(feature = "audio")]
            VmToHost::Sound(cmd) => {
                let _ = self.singer.send(cmd);
            }
        }
    }

    fn update(&mut self, catcher: &Receiver<MainPacket>) -> Option<UpdateOut> {
        // A finished mic capture has already shipped its sample to the synth, so
        // drop the input stream to release the microphone right away instead of
        // sitting on it (and on the OS's "mic in use" indicator).
        #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
        if self.mic_stream.is_some()
            && self.mic_done.load(std::sync::atomic::Ordering::Acquire)
        {
            self.mic_stream = None;
            self.log(LogType::Config, "mic: capture complete");
        }

        let mut loop_complete = false;
        let mut only_one_gui_sync = true;
        catcher.try_iter().for_each(|(id, p)| {
            // println!("{} {}", "[ 2 ]".on_bright_purple(), "core update loop");
            match p {
                MainCommmand::Cam(p, r) => {
                    // A camera belongs to the bundle that set it. This used to write
                    // straight into the one global camera, so an overlay calling `cam`
                    // swung the app's view out from under it — and an overlay wanting
                    // its own 3D space has to call `cam` by definition.
                    self.bundle_manager.set_camera(id, p, r);
                    // The scene is still drawn from the app's camera; an overlay's is
                    // recorded for its own pass (see PLAN.md, overlay 3D) and does not
                    // touch the view.
                    if !self.bundle_manager.is_overlay(id) {
                        if let Some(pos) = p {
                            self.global.cam_pos = pos;
                        }
                        if let Some(rot) = r {
                            self.global.simple_cam_rot = rot;
                        }
                    }
                }
                MainCommmand::Light(dir, color, ambient, sky, ground) => {
                    if let Some(d) = dir {
                        self.global.light_dir = d;
                    }
                    if let Some(c) = color {
                        self.global.light_color = c;
                    }
                    if let Some(a) = ambient {
                        self.global.light_ambient = a;
                    }
                    // Any hemisphere colour switches ambient to hemisphere mode
                    // (amb_sky.w = 1); both default to the given/zero colour.
                    if sky.is_some() || ground.is_some() {
                        let s = sky.or(ground).unwrap_or(glam::Vec3::ZERO);
                        let g = ground.or(sky).unwrap_or(glam::Vec3::ZERO);
                        self.global.amb_sky = glam::vec4(s.x, s.y, s.z, 1.);
                        self.global.amb_ground = glam::vec4(g.x, g.y, g.z, 0.);
                    }
                }
                MainCommmand::Fog(v) => {
                    self.global.fog_color = v;
                }
                MainCommmand::MouseGrab(on) => {
                    // Desired state; the frame loop reconciles it against the
                    // actual grab (and defers to a click on web).
                    self.global.mouse_grab = on;
                }
                MainCommmand::GetImg(s, tx) => {
                    #[cfg(feature = "headed")]
                    self.log_check(tx.send(self.tex_manager.get_img(&s)));
                }
                MainCommmand::SetImg(s, im, tx) => {
                    #[cfg(feature = "headed")]
                    {
                        self.tex_manager.overwrite_texture(
                            &s,
                            im,
                            &mut self.world,
                            id,
                            &mut self.loggy,
                        );
                        self.log_check(tx.send(()));
                        self.tex_manager
                            .refinalize(&self.gfx.queue, &self.gfx.master_texture);
                    }
                }
                MainCommmand::Anim(name, items, speed) => {
                    #[cfg(feature = "headed")]
                    {
                        let frames = items
                            .iter()
                            .map(|i| self.tex_manager.get_tex(i))
                            .collect_vec();
                        if frames.len() == 0 {
                            self.loggy.log(
                                log::LogType::TextureError,
                                &format!("Animation {} has no frames, not storing", name),
                            );
                        } else {
                            self.tex_manager.animations.insert(
                                name,
                                crate::texture::Anim {
                                    frames,
                                    speed,
                                    once: false,
                                },
                            );
                        }
                    }
                }
                MainCommmand::Model(model) => {
                    let res = self.model_manager.upsert_model(
                        #[cfg(feature = "headed")]
                        &self.gfx.device,
                        #[cfg(feature = "headed")]
                        &self.tex_manager,
                        &mut self.world,
                        id,
                        &model.asset,
                        model.textures,
                        model.vecs,
                        model.norms,
                        model.inds,
                        model.uvs,
                        model.style,
                        &mut self.loggy,
                        self.global.debug,
                    );
                    if let Some(m) = res {
                        self.global.state_changes.push(StateChange::ModelChange(m));
                        self.global.is_state_changed = true;
                    }

                    self.log_check(model.sender.send(0));
                }
                MainCommmand::ListModel(s, bundles, tx) => {
                    let list = self.model_manager.search_model(&s, bundles);
                    self.log_check(tx.send(list));
                }
                MainCommmand::Make(m, tx) => {
                    if m.len() == 7 {
                        // change order to match expectations from the front end
                        let m2 = vec![
                            m[1].clone(),
                            m[6].clone(),
                            m[2].clone(),
                            m[4].clone(),
                            m[3].clone(),
                            m[5].clone(),
                        ];
                        self.model_manager.edit_cube(
                            #[cfg(feature = "headed")]
                            &self.gfx.device,
                            #[cfg(feature = "headed")]
                            &self.tex_manager,
                            &mut self.world,
                            id,
                            m[0].clone(),
                            m2,
                        );

                        self.log_check(tx.send(0));
                    }
                }
                MainCommmand::Spawn(lent) => {
                    // Native/headless: the in-process VM sends the shared wrapper.
                    // On wasm the VM is in a worker and spawns arrive as
                    // VmToHost::Spawn(LuaEnt) applied elsewhere, so this
                    // wrapper-based path is native-only.
                    #[cfg(not(target_arch = "wasm32"))]
                    self.ent_manager.create_from_lua(
                        #[cfg(feature = "headed")]
                        &self.tex_manager,
                        #[cfg(feature = "headed")]
                        &self.model_manager,
                        lent,
                    );
                    #[cfg(target_arch = "wasm32")]
                    let _ = lent;
                }
                MainCommmand::Group(parent, child, tx) => {
                    self.ent_manager.group(parent, child);
                    self.log_check(tx.send(true));
                }
                MainCommmand::Globals(table) => {
                    for (k, v) in table.iter() {
                        #[cfg(feature = "headed")]
                        match k.as_str() {
                            "resolution" => {
                                self.global.screen_effects.crt_resolution = Self::val2float(v)
                            }
                            "curvature" => {
                                self.global.screen_effects.corner_harshness = Self::val2float(v)
                            }
                            "flatness" => {
                                self.global.screen_effects.corner_ease = Self::val2float(v)
                            }
                            "dark" => self.global.screen_effects.dark_factor = Self::val2float(v),
                            "bleed" => {
                                self.global.screen_effects.lumen_threshold = Self::val2float(v)
                            }
                            "glitch" => self.global.screen_effects.glitchiness = Self::val2vec3(v),
                            "high" => self.global.screen_effects.high_range = Self::val2float(v),
                            "low" => self.global.screen_effects.low_range = Self::val2float(v),
                            "modernize" => {
                                self.global.screen_effects.modernize = Self::val2float(v)
                            }
                            "fog" => self.global.screen_effects.fog = Self::val2float(v),
                            "fullscreen" => {
                                self.global.fullscreen = Self::val2bool(v);
                                self.check_fullscreen();
                                self.global.fullscreen_state = self.global.fullscreen;
                            }
                            "mouse_grab" => self.global.mouse_grab = Self::val2bool(v),
                            "size" => {
                                let arr = Self::val2array(v);

                                self.gfx.set_window_size(arr.get(0), arr.get(1));
                            }
                            "title" => {
                                if let Some(s) = Self::val2string(v) {
                                    self.gfx.set_title(&s);
                                }
                            }
                            // Honour the value like every other boolean attr: this
                            // used to lock unconditionally, so `attr{lock=false}`
                            // still locked and nothing could ever unlock — leaving
                            // the console unreachable for the rest of the session.
                            "lock" => {
                                let lock = Self::val2bool(v);
                                self.global.locked = lock;
                                self.global.console = !lock;
                                if lock {
                                    self.gui.disable_console();
                                } else {
                                    self.gui.enable_console(&self.loggy);
                                }
                            }

                            _ => {}
                        }
                        #[cfg(not(feature = "headed"))]
                        match k.as_str() {
                            "lock" => {
                                let lock = Self::val2bool(v);
                                self.global.locked = lock;
                                self.global.console = !lock;
                            }
                            _ => {}
                        }
                    }
                }
                MainCommmand::GetGlobal(tx) => {
                    #[cfg(feature = "headed")]
                    let resolution = self.global.gui_params.resolution;
                    #[cfg(not(feature = "headed"))]
                    let resolution = (256, 256);
                    let t = GlobalMap::new(OS, 60., resolution);
                    self.log_check(tx.send(t));
                }
                MainCommmand::AsyncError(e) => {
                    let s = format!("!!{}", e);
                    self.log(log::LogType::LuaError, &s);
                }
                #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
                MainCommmand::MicRecord(id, secs, tx) => {
                    // cpal streams aren't Send, so the input stream is built and
                    // owned here on the main thread. One capture at a time.
                    let busy = self.mic_stream.is_some();
                    let reply = match id {
                        // Query only — must never open the microphone.
                        None => busy,
                        Some(_) if busy => false,
                        Some(id) => {
                            self.mic_done
                                .store(false, std::sync::atomic::Ordering::Release);
                            match crate::sound::record_mic(
                                id,
                                secs,
                                self.singer.clone(),
                                self.mic_done.clone(),
                            ) {
                                Ok(stream) => {
                                    self.mic_stream = Some(stream);
                                    true
                                }
                                Err(e) => {
                                    self.log(LogType::ConfigError, &format!("!!mic: {}", e));
                                    false
                                }
                            }
                        }
                    };
                    self.log_check(tx.send(reply));
                }
                #[cfg(not(all(feature = "audio", not(target_arch = "wasm32"))))]
                MainCommmand::MicRecord(_, _, tx) => {
                    self.log_check(tx.send(false));
                }
                MainCommmand::Read(path, tx) => {
                    println!("read {}", path);
                    let pak = match self.bundle_manager.get_main_bundle().get_directory() {
                        Some(dir) => match crate::file_util::get_file_string_scrubbed(dir, &path) {
                            Ok(s) => Some(s),
                            Err(e) => {
                                self.log(LogType::IoError, &format!("!!{}", e));
                                None
                            }
                        },
                        None => {
                            self.log(LogType::IoError, &format!("!! No relative path access"));
                            None
                        }
                    };
                    let res = tx.send(pak);
                    self.log_check(res);
                }
                MainCommmand::Write(file, contents, tx) => {
                    let res = match self.bundle_manager.get_main_bundle().get_directory() {
                        Some(dir) => {
                            if let Err(e) =
                                crate::file_util::write_file_string_scrubbed(dir, &file, &contents)
                            {
                                self.log(LogType::IoError, &format!("!!{}", e));
                                false
                            } else {
                                true
                            }
                        }
                        None => {
                            self.log(LogType::IoError, &format!("!! No relative path access"));
                            false
                        }
                    };
                    self.log_check(tx.send(res));
                }
                // The `app.*` family: an overlay reaching into the app beneath it.
                //
                // Every arm resolves the target through `overlay_edit_target`, which
                // returns nothing unless the sender is an engine-marked overlay. The
                // natives don't exist in a game's VM to begin with, but this is the
                // door to another app's files, so the main thread checks the caller
                // itself rather than trusting that the door was never installed.
                MainCommmand::AppRead(path, tx) => {
                    let out = match self.overlay_edit_target(id) {
                        Some((_, dir)) => {
                            match crate::file_util::get_file_string_scrubbed(&dir, &path) {
                                Ok(s) => Some(s),
                                Err(e) => {
                                    self.log(LogType::IoError, &format!("!!{}", e));
                                    None
                                }
                            }
                        }
                        None => {
                            self.log(LogType::IoError, "!!app.read: not an overlay");
                            None
                        }
                    };
                    self.log_check(tx.send(out));
                }
                MainCommmand::AppWrite(file, contents, tx) => {
                    let res = match self.overlay_edit_target(id) {
                        Some((_, dir)) => match crate::file_util::write_file_string_scrubbed(
                            &dir, &file, &contents,
                        ) {
                            Ok(()) => true,
                            Err(e) => {
                                self.log(LogType::IoError, &format!("!!{}", e));
                                false
                            }
                        },
                        None => {
                            self.log(LogType::IoError, "!!app.write: not an overlay");
                            false
                        }
                    };
                    self.log_check(tx.send(res));
                }
                MainCommmand::AppList(tx) => {
                    let out = match self.overlay_edit_target(id) {
                        Some((_, dir)) => match crate::file_util::list_files_scrubbed(&dir) {
                            Ok(v) => v,
                            Err(e) => {
                                self.log(LogType::IoError, &format!("!!{}", e));
                                vec![]
                            }
                        },
                        None => {
                            self.log(LogType::IoError, "!!app.list: not an overlay");
                            vec![]
                        }
                    };
                    self.log_check(tx.send(out));
                }
                MainCommmand::AppReload(tx) => {
                    let res = match self.overlay_edit_target(id) {
                        Some((target, _)) => {
                            crate::command::reload(self, target);
                            true
                        }
                        None => {
                            self.log(LogType::IoError, "!!app.reload: not an overlay");
                            false
                        }
                    };
                    self.log_check(tx.send(res));
                }
                MainCommmand::BundleDropped(b) => {
                    self.completed_bundles.remove(&id);
                    self.bundle_manager.reclaim_resources(b);
                }
                MainCommmand::Subload(file, is_overlay) => {
                    if let Err(e) = crate::command::load_app(
                        self,
                        Some(file.as_str()),
                        None,
                        None,
                        Some((id, is_overlay)),
                    ) {
                        self.log(LogType::LuaError, &format!("!!{}", e))
                    };
                }
                MainCommmand::Reload() => crate::command::reload(self, id),

                MainCommmand::WorldSync(chunks, dropped) => {
                    self.world.process_sync(
                        #[cfg(feature = "headed")]
                        &self.gfx.device,
                        id,
                        chunks,
                        dropped,
                        &self.model_manager,
                    );
                }
                MainCommmand::Stats() => {
                    self.world.stats();
                }
                MainCommmand::Quit(u) => {
                    if u > 0 {
                        // Take (read + clear) the queued load. The clear must NOT
                        // precede the read — a stray `pending_load = None` here
                        // used to null it first, so every arg/dropped .game.png
                        // fell through to load_empty and "no bundle" loaded.
                        match self.global.pending_load.take() {
                            Some(to_load) => {
                                self.log(LogType::Sys, &format!("load {}", to_load));
                                crate::command::hard_reset(self);

                                crate::command::load_app(self, Some(&to_load), None, None, None);
                            }
                            _ => {
                                //DEV if a load quit is triggered and then the lua context spams it too fast it technically quits to empty comnsole. should ahve it be code based or not trigger too quickly
                                crate::command::hard_reset(self);
                                crate::command::load_empty(self);
                            }
                        }
                    } else {
                        self.global.state_changes.push(StateChange::Quit);
                    }
                    self.global.is_state_changed = true;
                }
                MainCommmand::InitBack(refs) => {
                    let (main_ref, sky_ref) = *refs;
                    self.bundle_manager.set_img_refs(id, main_ref, sky_ref);
                }
                MainCommmand::LoopComplete(mutations) => {
                    if mutations.gui {
                        #[cfg(feature = "headed")]
                        self.gui.mark_bundle_dirty(id, false);
                    }
                    if mutations.sky {
                        #[cfg(feature = "headed")]
                        self.gui.mark_bundle_dirty(id, true);
                    }
                    self.completed_bundles.insert(id, true);
                    loop_complete = true;
                }
                MainCommmand::Copy(s) => {
                    #[cfg(desktop)]
                    if let Ok(mut ctx) = ClipboardContext::new() {
                        if let Err(_) = ctx.set_contents(s) {
                            self.log(LogType::IoError, &format!("!!Clipboard error"));
                        }
                    }
                    #[cfg(target_arch = "wasm32")]
                    let _ = s;
                }
                MainCommmand::LuaClose() => {
                    // println!("close lua channel {}",id);
                    // self.bundle_manager.close_lua_channel(id);
                }
                MainCommmand::Load(_) => todo!(),
                MainCommmand::Null() => todo!(),
            }
        });

        #[cfg(feature = "headed")]
        let instance_buffers = if loop_complete {
            Some(self.ent_manager.check_ents(
                &self.gfx.device,
                &self.tex_manager,
                &self.model_manager,
                self.global.iteration,
            ))
        } else {
            None
        };
        #[cfg(not(feature = "headed"))]
        let instance_buffers = {
            if loop_complete {
                self.ent_manager.check_ents(self.global.iteration);
            }
            None
        };

        self.global.iteration += 1;
        instance_buffers
    }
}

pub fn error_window(e: Box<dyn std::error::Error>) {
    // #[cfg(target_os = "windows")]
    // {
    //     use std::ptr::null_mut as NULL;
    //     use winapi::um::winuser;
    //     let st = format!("{}\0", e.to_string());
    //     let l_msg: Vec<u16> = st.encode_utf16().collect();
    //     // let l_msg: Vec<u16> = "Wassa wassa wassup\0".encode_utf16().collect();
    //     let l_title: Vec<u16> = "Petrichor64 Error\0".encode_utf16().collect();

    //     unsafe {
    //         winuser::MessageBoxW(
    //             NULL(),
    //             l_msg.as_ptr(),
    //             l_title.as_ptr(),
    //             winuser::MB_OK | winuser::MB_ICONINFORMATION,
    //         );
    //     }
    // }
    #[cfg(desktop)]
    native_dialog::DialogBuilder::message()
        .set_level(native_dialog::MessageLevel::Error)
        .set_title("Petrichor64 Error")
        // .set_text(&format!("{:#?}", path))
        .set_text(&e.to_string())
        .confirm()
        .show()
        .unwrap();
    // No native dialog on the web; surface the error to the JS console instead.
    #[cfg(target_arch = "wasm32")]
    web_sys::console::error_1(&format!("Petrichor64 Error: {}", e).into());
    // Phones have no modal-dialog crate we can call from here (and no console the
    // user can see). Log it: on Android this lands in `adb logcat`, which is where
    // you'd be looking anyway.
    #[cfg(all(not(desktop), not(target_arch = "wasm32")))]
    ::log::error!("Petrichor64 Error: {}", e);
}

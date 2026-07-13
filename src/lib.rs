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
use clipboard::{ClipboardContext, ClipboardProvider};
use colored::Colorize;
#[cfg(feature = "headed")]
use ent_manager::InstanceBuffer;
use glam::vec2;
use global::StateChange;
use gui::ScreenIndex;
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
mod sound;
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
    /// wasm builds init the engine asynchronously (wgpu adapter/device requests
    /// can't block the browser main thread). `resumed` kicks off the build via
    /// spawn_local and drops the finished Core here; the frame loop installs it
    /// once ready. Rc<RefCell<…>> is fine — wasm is single-threaded.
    #[cfg(target_arch = "wasm32")]
    pending_core: std::rc::Rc<std::cell::RefCell<Option<(Core, Receiver<MainPacket>)>>>,
    /// The VM web worker (wasm only). Spawned once; drives the Lua VM off-thread.
    #[cfg(target_arch = "wasm32")]
    worker: Option<crate::web_worker::WorkerHandle>,
    /// Whether the worker has been sent its Init + initial Load.
    #[cfg(target_arch = "wasm32")]
    worker_inited: bool,
    /// Shared "app wants the mouse grabbed" flag (mirrors global.mouse_grab).
    /// The canvas mousedown handler reads it to decide whether to request
    /// pointer-lock (which browsers only grant from a user gesture).
    #[cfg(target_arch = "wasm32")]
    pointer_lock_wanted: std::rc::Rc<std::cell::RefCell<bool>>,
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
            #[cfg(target_arch = "wasm32")]
            pending_core: std::rc::Rc::new(std::cell::RefCell::new(None)),
            #[cfg(target_arch = "wasm32")]
            worker: None,
            #[cfg(target_arch = "wasm32")]
            worker_inited: false,
            #[cfg(target_arch = "wasm32")]
            pointer_lock_wanted: std::rc::Rc::new(std::cell::RefCell::new(false)),
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

        // --- Native: build Core synchronously and load the default app. ---
        #[cfg(not(target_arch = "wasm32"))]
        {
            let (mut core, catcher) = pollster::block_on(Core::new(window.clone()));
            self.catcher = Some(catcher);

            crate::command::load_empty(&mut core);
            {
                crate::command::hard_reset(&mut core);
                if let Err(e) =
                    crate::command::load_app(&mut core, Some("test/basic"), None, None, None)
                {
                    core.loggy.log(LogType::CoreError, &format!("{}", e));
                }
            }
            core.loggy.clear();

            core.global.state_changes.push(StateChange::Config);
            // Small delay so the console-app's pending requests finish before
            // the following config state change fires.
            core.global.state_delay = 8;
            core.global.is_state_changed = true;

            // --- Auto-load or command-line file ---
            let maybe_load = if env::args().count() > 1 {
                Some(env::args().nth(1).unwrap())
            } else {
                crate::asset::check_for_auto()
            };

            if let Some(s) = maybe_load {
                core.global.console = false;
                core.gui.disable_console();
                core.global.pending_load = Some(s.clone());
                core.bundle_manager.get_lua().call_drop(s);
            } else {
                #[cfg(feature = "include_auto")]
                {
                    core.global.console = false;
                    core.gui.disable_console();
                    let _id = core.bundle_manager.console_bundle_target;
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
                install_web_input_handlers(&canvas, self.pointer_lock_wanted.clone());
            }
            let pending = self.pending_core.clone();
            wasm_bindgen_futures::spawn_local(async move {
                let (core, catcher) = Core::new(window).await;
                ::log::info!("petrichor64: core ready — wgpu initialised");
                *pending.borrow_mut() = Some((core, catcher));
            });
        }
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
                let s = mods.state();
                let b = &mut self.bits.0;
                b[KeyCode::ShiftLeft as usize] = s.shift_key();
                b[KeyCode::ShiftRight as usize] = s.shift_key();
                b[249] = s.shift_key();
                b[KeyCode::ControlLeft as usize] = s.control_key();
                b[KeyCode::ControlRight as usize] = s.control_key();
                b[248] = s.control_key();
                b[KeyCode::SuperLeft as usize] = s.super_key();
                b[KeyCode::SuperRight as usize] = s.super_key();
                b[250] = s.super_key();
                b[KeyCode::AltLeft as usize] = s.alt_key();
                b[KeyCode::AltRight as usize] = s.alt_key();
                b[247] = s.alt_key();
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
        // Install the asynchronously-built Core once it's ready (web only).
        #[cfg(target_arch = "wasm32")]
        if self.core.is_none() {
            if let Some((core, catcher)) = self.pending_core.borrow_mut().take() {
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
            let mut drained: Vec<crate::worker_protocol::VmToHost> = Vec::new();
            if let Some(w) = &self.worker {
                if w.is_ready() {
                    if !self.worker_inited {
                        // Load the payload bundle's loaded assets into the atlas
                        // (disjoint field from self.worker), then send the app's
                        // main.lua and call main(). Bundle scripts/assets are
                        // embedded for now; proper .game.png unpack on wasm is a
                        // follow-up.
                        const PAYLOAD_MAIN: &str = include_str!("../payload/scripts/main.lua");
                        const PAYLOAD_EXAMPLE: &[u8] =
                            include_bytes!("../payload/assets/example.png");
                        if let Some(core) = self.core.as_mut() {
                            core.load_wasm_texture("example", PAYLOAD_EXAMPLE);
                            // Set up bundle 0's main-side world (GPU meshing +
                            // mapper) without a world thread; the worker owns the
                            // tile data and syncs chunks here.
                            core.world.init_local(0);
                        }
                        w.post(&HostToVm::Init {
                            bundle_id: 0,
                            width: 256,
                            height: 256,
                        });
                        w.post(&HostToVm::Load {
                            name: "main".to_string(),
                            content: PAYLOAD_MAIN.to_string(),
                        });
                        w.post(&HostToVm::Main);
                        self.worker_inited = true;
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
                    // With the console open, the app must not receive input — the
                    // keys are going to the console. Send a neutral snapshot.
                    let console_open = self.core.as_ref().map_or(false, |c| c.global.console);
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
            for m in drained {
                if let Some(core) = self.core.as_mut() {
                    core.apply_vm_message(m);
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
            core.bundle_manager
                .call_loop(&mut core.completed_bundles, &self.bits);

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

#[cfg(all(feature = "headed", not(target_arch = "wasm32")))]
pub fn start() {
    env_logger::init();

    let event_loop = match EventLoop::<()>::new() {
        Ok(el) => el,
        Err(e) => {
            error_window(Box::new(e));
            return;
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
) {
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen::JsCast;

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

    crate::command::load_empty(&mut core);
    crate::command::hard_reset(&mut core);
    if let Err(e) = crate::command::load_app(&mut core, Some("test/basic"), None, None, None) {
        core.loggy.log(LogType::CoreError, &format!("{}", e));
    }

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
        }
    }

    fn update(&mut self, catcher: &Receiver<MainPacket>) -> Option<UpdateOut> {
        let mut loop_complete = false;
        let mut only_one_gui_sync = true;
        catcher.try_iter().for_each(|(id, p)| {
            // println!("{} {}", "[ 2 ]".on_bright_purple(), "core update loop");
            match p {
                MainCommmand::Cam(p, r) => {
                    if let Some(pos) = p {
                        self.global.cam_pos = pos;
                    }
                    if let Some(rot) = r {
                        self.global.simple_cam_rot = rot;
                    }
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
                    println!("make heard!");
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
                            "lock" => {
                                self.global.console = false;
                                self.gui.disable_console();
                                self.global.locked = true;
                            }

                            _ => {}
                        }
                        #[cfg(not(feature = "headed"))]
                        match k.as_str() {
                            "lock" => {
                                self.global.console = false;
                                self.global.locked = true;
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
                        // println!(
                        //     "quit with pending load {} {:?}",
                        //     u, self.global.pending_load
                        // );
                        self.global.pending_load = None;
                        match &self.global.pending_load {
                            Some(l) => {
                                let to_load = l.clone();
                                self.log(LogType::Sys, &format!("load {}", l));
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
                        self.gui.mark_dirty(ScreenIndex::Primary, id);
                    }
                    if mutations.sky {
                        #[cfg(feature = "headed")]
                        self.gui.mark_dirty(ScreenIndex::Sky, id);
                    }
                    self.completed_bundles.insert(id, true);
                    loop_complete = true;
                }
                MainCommmand::Copy(s) => {
                    #[cfg(not(target_arch = "wasm32"))]
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
    #[cfg(not(target_arch = "wasm32"))]
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
}

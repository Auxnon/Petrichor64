//! One running engine: a cart, its VM, and everything it draws.
//!
//! An `Instance` never opens a window. A host hands it input
//! ([`Instance::input`]), asks it to advance ([`Instance::frame`]) and to draw
//! ([`Instance::render`]), and takes what it wanted of a window
//! ([`Instance::requests`]). `chor` is the first host: a winit window around
//! one instance drawn to the window's surface (`App` in `lib.rs`). Another host
//! makes its own with [`Instance::new`], on its own wgpu device, and samples
//! [`Instance::texture`] wherever it likes; several can run side by side.

use std::sync::mpsc::Receiver;
use std::time::Duration;

use glam::vec2;
use winit::dpi::PhysicalSize;
use winit::event::ElementState;
use winit::keyboard::KeyCode;

use crate::controls;
use crate::gfx::{Gfx, Target};
use crate::global::StateChange;
use crate::host::{Button, Cart, HostGpu, Input, Options, Request, TouchPhase};
use crate::lua_define::MainPacket;
use crate::render::DrawState;
use crate::types::ControlState;
use crate::Core;

pub struct Instance {
    pub(crate) core: Core,
    pub(crate) catcher: Receiver<MainPacket>,
    /// Current key/mouse-button state sent to Lua each tick.
    pub(crate) bits: ControlState,
    /// Key state from the previous tick, for pressed/released detection.
    pub(crate) bits_prev: [bool; 256],
    /// The finger acting as the cursor, by touch id. `None` when nothing is
    /// touching.
    primary_touch: Option<u64>,
    /// Last primary-touch position in pixels, for drag deltas.
    touch_last: Option<(f32, f32)>,
    /// What input positions are measured against: the instance's rect in
    /// physical pixels, as the host last gave it.
    size: (u32, u32),
    /// When `frame` next runs a tick.
    next_tick: Option<Duration>,
    /// A cart has been loaded, so the next `load` clears it out first.
    loaded: bool,
}

impl Instance {
    /// An instance on the host's device, running `opts.cart`. It draws into a
    /// texture of its own ([`Instance::texture`]) in the host's format.
    ///
    /// The device is the host's to watch. If it's lost, make new instances on
    /// the new one (the carts restart).
    // TODO: an `Instance::rebuild(&HostGpu)` that keeps the VM, as
    // `Core::restore_gpu` does for `chor`.
    //
    // Not on the web yet: there the VM runs in a worker that `App` drives, and
    // a page gets several engines by starting several (see the web element).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(gpu: &HostGpu, opts: Options) -> Result<Self, String> {
        let link = Gfx::host_link(gpu, opts.size, opts.scale as f64);
        let (mut core, catcher) = Core::build(link, opts.policy);
        // The boot console is `chor`'s; a host opens it if it wants one.
        core.global.console = false;
        core.gui.disable_console();
        let mut inst = Self::wrap(core, catcher);
        inst.resize(opts.size, opts.scale);
        inst.load(opts.cart)?;
        Ok(inst)
    }

    /// Around a core that already has somewhere to draw.
    pub(crate) fn wrap(core: Core, catcher: Receiver<MainPacket>) -> Self {
        let size = (core.gfx.size.width, core.gfx.size.height);
        Self {
            core,
            catcher,
            bits: ControlState::default(),
            bits_prev: [false; 256],
            primary_touch: None,
            touch_last: None,
            size,
            next_tick: None,
            loaded: false,
        }
    }

    /// Run this cart, in place of whatever was running.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn load(&mut self, cart: Cart) -> Result<(), String> {
        let core = &mut self.core;
        if self.loaded {
            crate::command::hard_reset(core);
            crate::drain_stray_messages(&self.catcher);
        }
        self.loaded = true;
        let res = match cart {
            Cart::None => {
                crate::command::load_empty(core);
                core.loggy.clear();
                Ok(())
            }
            Cart::Path(path) => {
                crate::command::load_app(core, Some(&path.to_string_lossy()), None, None, None)
            }
            Cart::Bytes(bytes) => {
                crate::command::load_app(core, Some("cart"), Some(bytes), None, None)
            }
        };
        res.map_err(|e| e.to_string())
    }

    /// The rect the instance is shown in changed: `size` in physical pixels,
    /// `scale` the display's scale factor. A hosted instance's texture is
    /// remade at once, so take [`Instance::texture`] again after this.
    pub fn resize(&mut self, size: (u32, u32), scale: f32) {
        if size.0 == 0 || size.1 == 0 {
            return;
        }
        self.size = size;
        self.core.gfx.set_scale(scale as f64);
        let size = PhysicalSize::new(size.0, size.1);
        if matches!(self.core.gfx.target, Target::Texture { .. }) {
            if self.core.gfx.size != size {
                self.core.gfx.set_config_size(size);
                self.core.debounced_resize();
            }
        } else {
            // A window being dragged sends a stream of these: debounced.
            self.core.resize(size);
        }
    }

    /// The instance's rect in physical pixels, as last given to `resize`.
    pub fn size(&self) -> (u32, u32) {
        self.size
    }

    /// Hand the instance one piece of input. Give it only to the instance
    /// that has focus, and `Input::FocusLost` to the one that lost it.
    pub fn input(&mut self, ev: Input) {
        let (w, h) = (self.size.0.max(1) as f32, self.size.1.max(1) as f32);
        let core = &mut self.core;
        // The gear and command strip take a press before the app can.
        #[cfg(feature = "studio")]
        {
            let at = match &ev {
                Input::Button {
                    button: Button::Left,
                    pressed: true,
                } => Some((core.global.mouse_pos.x, core.global.mouse_pos.y)),
                Input::Touch {
                    phase: TouchPhase::Start,
                    x,
                    y,
                    ..
                } => Some((x / w, y / h)),
                _ => None,
            };
            if let Some((x, y)) = at {
                if core.strip_press(x, y) {
                    return;
                }
            }
        }
        match ev {
            Input::Key { code, pressed } => {
                let state = if pressed {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                };
                controls::bit_check(&state, code, &mut self.bits);
                // Straight into the console's line, so it follows key repeat
                // rather than the tick.
                if pressed && code == KeyCode::Backspace && core.global.console {
                    core.loggy.back();
                }
            }
            Input::Text(text) => {
                // The backtick opens and closes the console.
                if core.global.console && text != "`" {
                    core.loggy.add(&text);
                }
            }
            // Only the four collapsed modifier slots, which `bit_check` also
            // writes for either side's key.
            Input::Modifiers {
                alt,
                ctrl,
                shift,
                logo,
            } => {
                let b = &mut self.bits.0;
                b[247] = alt;
                b[248] = ctrl;
                b[249] = shift;
                b[250] = logo;
            }
            Input::PointerMoved { x, y } => {
                core.global.mouse_pos.x = x / w;
                core.global.mouse_pos.y = y / h;
            }
            // Summed: several can land between two ticks, and the tick resets
            // it after Lua has read it.
            Input::PointerDelta { dx, dy } => {
                core.global.mouse_delta += vec2(dx, dy);
            }
            Input::Button { button, pressed } => {
                let val = if pressed { 1.0 } else { 0.0 };
                let i = match button {
                    Button::Left => 0,
                    Button::Right => 1,
                    Button::Middle => 2,
                    Button::Forward => 3,
                };
                core.global.mouse_buttons[i] = val;
            }
            // Summed: a frame can see several wheel events (a fast spin, a
            // touchpad).
            Input::Wheel { x, y } => {
                core.global.scroll_delta.0 += x;
                core.global.scroll_delta.1 += y;
            }
            // Touch, folded into the mouse so every game and `mus()` call
            // works on a phone unchanged: the *primary* finger is the cursor,
            // and touching down is a left click.
            //
            // Primary means the first finger down that is still down, tracked
            // by touch id, not "whichever event arrived": a second finger
            // landing and lifting during a drag must not move the cursor or
            // release the button. Every finger, the primary included, is also
            // tracked for `touches()`, where a script reads gestures.
            Input::Touch {
                id,
                phase,
                x,
                y,
                pen,
                pressure,
            } => {
                let down = matches!(phase, TouchPhase::Start | TouchPhase::Move);
                core.global.track_touch(
                    crate::global::Touching {
                        id,
                        x: x / w,
                        y: y / h,
                        pen,
                        pressure,
                    },
                    down,
                );
                match phase {
                    TouchPhase::Start => {
                        if self.primary_touch.is_none() {
                            self.primary_touch = Some(id);
                            // No delta on the first contact: there's no previous
                            // position to be relative to, and inventing one makes
                            // a tap look like a flick to anything reading `mus`
                            // delta.
                            self.touch_last = Some((x, y));
                            core.global.mouse_pos.x = x / w;
                            core.global.mouse_pos.y = y / h;
                            core.global.mouse_buttons[0] = 1.0;
                        }
                    }
                    TouchPhase::Move => {
                        if self.primary_touch == Some(id) {
                            core.global.mouse_pos.x = x / w;
                            core.global.mouse_pos.y = y / h;
                            // Accumulated, in pixels, to match pointer deltas.
                            if let Some((lx, ly)) = self.touch_last {
                                core.global.mouse_delta.x += x - lx;
                                core.global.mouse_delta.y += y - ly;
                            }
                            self.touch_last = Some((x, y));
                        }
                    }
                    TouchPhase::End | TouchPhase::Cancel => {
                        if self.primary_touch == Some(id) {
                            self.primary_touch = None;
                            self.touch_last = None;
                            core.global.mouse_buttons[0] = 0.0;
                            // Position left where the finger lifted, like a mouse
                            // that stopped moving: games read it on release to
                            // decide what was hit.
                        }
                    }
                }
            }
            // A lost key-up (tab-away, a shortcut the OS took) would otherwise
            // leave keys stuck down.
            Input::FocusLost => {
                self.bits.0 = [false; 256];
            }
            // Fingers down as the app leaves won't send their lift; let them
            // go now, or they'd be stuck down (and the mouse held) on return.
            Input::Suspended => {
                self.primary_touch = None;
                self.touch_last = None;
                if core.global.touches.iter().any(|t| t.is_some()) {
                    core.global.touches = [None; crate::global::TOUCH_SLOTS];
                    core.global.mouse_buttons[0] = 0.0;
                }
            }
            Input::Drop(path) => {
                if core.global.boot_state && core.policy.load_apps {
                    core.global.pending_load = Some(path.clone());
                }
                let _ = core.bundle_manager.get_main_bundle().lua.call_drop(path);
            }
        }
    }

    /// Run a tick if one is due. `now` is the host's clock (any steady time
    /// since some start); the engine keeps its own 60 Hz against it. Returns
    /// whether a tick ran, which is when there's something new to draw.
    pub fn frame(&mut self, now: Duration) -> bool {
        let step = Duration::from_secs_f32(1.0 / crate::FPS);
        let due = *self.next_tick.get_or_insert(now);
        if now < due {
            return false;
        }
        self.tick();
        // Advance from when it was due, not from now (no drift). But well
        // behind (a stall, the host hidden), start over from now: catching up
        // a frame per call fast-forwards the game through the whole backlog.
        let mut next = due + step;
        if now.saturating_sub(next) > step * crate::MAX_FRAMES_BEHIND {
            next = now + step;
        }
        self.next_tick = Some(next);
        true
    }

    /// When `frame` will next run a tick, on the clock it was last given.
    pub fn next_frame(&self) -> Option<Duration> {
        self.next_tick
    }

    /// One engine tick, due or not: what the VM sent since the last one, the
    /// console and system keys, then this tick's input to the cart's `loop`.
    pub fn tick(&mut self) {
        let core = &mut self.core;
        let catcher = &self.catcher;

        #[cfg(not(target_arch = "wasm32"))]
        core.loop_helper.loop_start();

        state_changes(core, Some(catcher));

        // Let a grabbed pointer go while the console is open.
        if core.global.console && core.global.mouse_grabbed_state {
            core.requests.push(Request::CursorVisible(true));
            core.requests.push(Request::GrabPointer(false));
            core.global.mouse_grabbed_state = false;
        }

        // What the Lua threads sent. When a Lua loop completes, `update`
        // returns the freshly built entity batches for the renderer.
        if let Some(ib) = core.update(catcher) {
            core.instance_buffers = ib;
        }
        core.drop_unheard();

        // Bring the unprojected cursor up to date with this tick's pointer
        // before handing it to Lua: render, where the ray is otherwise
        // computed, doesn't run until after the Lua loop.
        core.refresh_cursor_ray();

        // The pointer and analogue state, into the float part of the bits.
        let g = &core.global;
        let a = &mut self.bits.1;
        a[0] = g.mouse_pos.x;
        a[1] = g.mouse_pos.y;
        a[2] = g.mouse_delta.x;
        a[3] = g.mouse_delta.y;
        a[4] = g.mouse_buttons[0];
        a[5] = g.mouse_buttons[1];
        a[6] = g.mouse_buttons[2];
        // mus().scroll: the wheel (vertical), else a sideways scroll
        a[7] = g.scroll_amount();
        a[8] = g.cursor_projected_pos.x;
        a[9] = g.cursor_projected_pos.y;
        a[10] = g.cursor_projected_pos.z;
        g.pack_touches(a);

        // System shortcuts and console keys.
        controls::controls_evaluate(core, &self.bits, &self.bits_prev, Some(catcher));

        #[cfg(feature = "studio")]
        core.strip_tick();

        // Per-tick deltas, now read.
        core.global.mouse_delta = vec2(0., 0.);
        core.global.scroll_delta = (0., 0.);

        // The Lua loop (~60 Hz). Nothing gets input while the console is
        // open: the keys are being typed into it, and must not also drive the
        // game.
        let quiet = ControlState::default();
        let feed = if core.global.console {
            &quiet
        } else {
            &self.bits
        };
        core.bundle_manager
            .call_loop(&mut core.completed_bundles, feed);

        self.bits_prev = self.bits.0;

        core.gfx.flush_unsubmitted();
    }

    /// Draw the frame into the instance's target and submit it.
    pub fn render(&mut self) {
        let _ = self.draw();
    }

    /// `render`, saying how it went: a window's surface can ask to be
    /// reconfigured, or have nothing to draw to this frame.
    pub(crate) fn draw(&mut self) -> DrawState {
        self.core.render()
    }

    /// The finished frame, for the host to sample. It's a new texture after a
    /// `resize`.
    ///
    /// Panics on an instance that draws to a window's surface, which only
    /// `chor` makes.
    pub fn texture(&self) -> &wgpu::TextureView {
        self.core
            .gfx
            .target_view()
            .expect("an instance on a window's surface has no texture")
    }

    /// What the cart wanted of a window since the last call. Grant or ignore
    /// each.
    pub fn requests(&mut self) -> Vec<Request> {
        let mut out = std::mem::take(&mut self.core.requests);
        out.append(&mut self.core.gfx.take_requests());
        out
    }

    /// host -> cart: a message for the cart's `host.recv()`. Kept until the
    /// cart reads it, even if it hasn't finished loading.
    pub fn post(&mut self, msg: String) {
        self.core.host_post(msg);
    }

    /// cart -> host: everything the cart `host.send`'d since the last call.
    pub fn messages(&mut self) -> Vec<String> {
        self.core.host_take()
    }

    /// Open or close the developer console (if the policy allows one).
    pub fn console(&mut self, open: bool) {
        let core = &mut self.core;
        if open && !core.policy.console {
            return;
        }
        core.global.console = open;
        if open {
            core.gui.enable_console(&core.loggy);
        } else {
            core.gui.disable_console();
        }
    }
}

/// The engine's queued state changes, once their delay has run out: resize,
/// pointer grab, quit, the boot config. Whatever needs a window becomes a
/// `Request` on the core.
fn state_changes(c: &mut Core, catcher: Option<&Receiver<MainPacket>>) {
    if c.global.is_state_changed {
        if c.global.state_delay > 0 {
            c.global.state_delay -= 1;
        } else {
            c.global.is_state_changed = false;
            let states: Vec<StateChange> = c.global.state_changes.drain(..).collect();
            for state in states {
                match state {
                    StateChange::MouseGrabOn => {
                        c.requests.push(Request::CursorVisible(false));
                        c.requests.push(Request::GrabPointer(true));
                        c.global.mouse_grabbed_state = true;
                    }
                    StateChange::MouseGrabOff => {
                        c.requests.push(Request::CursorVisible(true));
                        c.requests.push(Request::GrabPointer(false));
                        c.global.mouse_grabbed_state = false;
                    }
                    StateChange::Resized => {
                        c.debounced_resize();
                    }
                    StateChange::Quit => c.requests.push(Request::Quit),
                    // `chor`'s boot: the config file, then the command line.
                    // Nothing else queues it.
                    StateChange::Config => {
                        let res = crate::asset::parse_config(&mut c.global, &mut c.loggy);
                        if let Some(s) = res {
                            let _ = crate::command::run_con_sys(c, &s, catcher);
                        }

                        let args: Vec<String> = std::env::args().collect();
                        if args.len() > 1 {
                            let mut command = None;
                            for arg in args.iter() {
                                if arg.starts_with('-') {
                                    command = Some(arg.to_lowercase());
                                } else if command.is_some() {
                                    match command.unwrap().as_str() {
                                        "--init" | "-i" => {
                                            println!("cli-init: {:?}", arg);
                                            let _ = crate::command::run_con_sys(c, arg, catcher);
                                        }
                                        "--new" | "-n" => {
                                            println!("cli-new: {:?}", arg);
                                            let _ = crate::command::run_con_sys(
                                                c,
                                                &format!("new {}", arg),
                                                catcher,
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
                        c.ent_manager.check_for_model_change(&c.model_manager, &id);
                    }
                }
            }
            c.check_fullscreen();
        }
    }
    // Reconcile the cart's wanted pointer grab (`mgrab`) with what's been
    // asked for, each tick; let go while the console is open. The web build
    // defers to a canvas click instead (`pointer_lock_wanted`), browsers
    // granting pointer lock only from a gesture.
    #[cfg(not(target_arch = "wasm32"))]
    {
        let want = c.global.mouse_grab && !c.global.console;
        if want != c.global.mouse_grabbed_state {
            c.requests.push(Request::CursorVisible(!want));
            c.requests.push(Request::GrabPointer(want));
            c.global.mouse_grabbed_state = want;
        }
    }
}

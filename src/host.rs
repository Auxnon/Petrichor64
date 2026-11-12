//! Petrichor64 as a library: what a host app hands an engine instance, and
//! what it gets back. See `HANDOFF-host-library.md`.
//!
//! A host owns the window, the wgpu device and the event loop. Each
//! [`Instance`](crate::Instance) runs one cart on the host's device, draws into
//! a texture of its own, takes its input from the host, and asks the host
//! (through [`Request`]s) for anything a window would do. `chor` is one such
//! host: a winit window with a single instance drawn to its surface.

use std::path::PathBuf;

/// What a cart may reach beyond its own instance.
///
/// Enforced by construction: a Lua global that's off is never registered
/// (`init_lua_sys`), not checked at call time. The main thread also drops the
/// commands a policy forbids, in case one arrives anyway.
#[derive(Clone, Debug, PartialEq)]
pub struct Policy {
    /// `conn`: sockets and websockets.
    pub network: bool,
    /// `io.get` / `io.set` and friends, rooted here. `None`: no `io` at all.
    /// `Some("")` (an empty path) leaves paths as the engine resolves them
    /// today, which is what `chor` uses.
    pub files: Option<PathBuf>,
    /// `io.copy`: the system clipboard.
    pub clipboard: bool,
    /// The backtick console and its commands.
    pub console: bool,
    /// `load` / `sub` / `over` / `reset` to another cart.
    pub load_apps: bool,
    /// Sound out. Off: the sound natives still exist and do nothing.
    pub audio: bool,
    /// The host channel: Lua `host.send` / `host.recv`.
    pub host_messages: bool,
}

impl Policy {
    /// Everything allowed, as a cart run by `chor` has always had.
    pub fn open() -> Self {
        Self {
            network: true,
            files: Some(PathBuf::new()),
            clipboard: true,
            console: true,
            load_apps: true,
            audio: true,
            host_messages: true,
        }
    }

    /// A cart from someone else: it draws, plays sound and takes input, and
    /// reaches nothing outside its instance.
    pub fn sandbox() -> Self {
        Self {
            network: false,
            files: None,
            clipboard: false,
            console: false,
            load_apps: false,
            audio: true,
            host_messages: false,
        }
    }
}

impl Default for Policy {
    fn default() -> Self {
        Self::open()
    }
}

/// The cart an instance runs.
#[derive(Clone, Debug)]
pub enum Cart {
    /// A packed game (`.game.png` or zip), in memory. Nothing is written to disk.
    Bytes(Vec<u8>),
    /// A game folder or a packed game file on disk.
    Path(PathBuf),
    /// No cart: the engine's built-in boot app.
    None,
}

/// Something a cart wants that only the host can do. The host may grant or
/// ignore each one.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    /// Confine/lock the pointer to the instance (`true`) or let it go.
    GrabPointer(bool),
    CursorVisible(bool),
    Title(String),
    Fullscreen(bool),
    /// `attr{size={w, h}}`, in logical points; a missing one keeps that dimension.
    Size(Option<f32>, Option<f32>),
    Quit,
    /// Put this text on the system clipboard.
    Clipboard(String),
}

/// Which pointer button, as `mus()` numbers them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button {
    Left,
    Right,
    Middle,
    Forward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TouchPhase {
    Start,
    Move,
    End,
    Cancel,
}

/// Input the host hands an instance. Positions are in the instance's own
/// physical pixels, from its top left: the host subtracts its rect.
#[cfg(feature = "headed")]
#[derive(Clone, Debug)]
pub enum Input {
    /// A physical key going down (repeats included) or up.
    Key {
        code: winit::keyboard::KeyCode,
        pressed: bool,
    },
    /// Typed text, for the console.
    Text(String),
    /// The modifier keys as they are now (self-heals a lost key-up).
    Modifiers {
        alt: bool,
        ctrl: bool,
        shift: bool,
        logo: bool,
    },
    PointerMoved {
        x: f32,
        y: f32,
    },
    /// Relative pointer motion in pixels, for a grabbed pointer.
    PointerDelta {
        dx: f32,
        dy: f32,
    },
    Button {
        button: Button,
        pressed: bool,
    },
    /// Scroll, in lines.
    Wheel {
        x: f32,
        y: f32,
    },
    Touch {
        id: u64,
        phase: TouchPhase,
        x: f32,
        y: f32,
        pen: bool,
        pressure: f32,
    },
    /// The instance lost focus: every key is let go.
    FocusLost,
    /// The host went to the background: fingers are lifted.
    Suspended,
    /// A file dropped on the instance.
    Drop(String),
}

/// What the host lends: its device and queue, and the format of the texture
/// it will sample (what its own pipeline expects, e.g. `Rgba8UnormSrgb`).
#[cfg(feature = "headed")]
#[derive(Clone)]
pub struct HostGpu {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub format: wgpu::TextureFormat,
}

#[cfg(feature = "headed")]
#[derive(Clone, Debug)]
pub struct Options {
    pub cart: Cart,
    /// Physical pixels of the rect the instance is shown in.
    pub size: (u32, u32),
    /// The display's scale factor.
    pub scale: f32,
    pub policy: Policy,
}

#[cfg(feature = "headed")]
impl Default for Options {
    fn default() -> Self {
        Self {
            cart: Cart::None,
            size: (640, 548),
            scale: 1.0,
            policy: Policy::open(),
        }
    }
}

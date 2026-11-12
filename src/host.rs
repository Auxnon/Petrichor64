//! Petrichor64 as a library: what a host app hands an engine instance, and
//! what it gets back. See `HANDOFF-host-library.md`.
//!
//! A host owns the window, the wgpu device and the event loop. Each
//! [`Instance`](crate::Instance) runs one cart on the host's device, draws into
//! a texture of its own, takes its input from the host, and asks the host
//! (through [`Request`]s) for anything a window would do. `chor` is one such
//! host: a winit window with a single instance drawn to its surface.

use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// What a cart may reach beyond its own instance.
///
/// Enforced by construction: a Lua global that's off is never registered
/// (`init_lua_sys`), not checked at call time. The main thread also drops the
/// commands a policy forbids, in case one arrives anyway
/// (`MainCommmand::allowed_by`, `VmToHost::allowed_by`).
///
/// What each field gates:
/// - `network`: the `conn` global.
/// - `files`: the `io` table (`io.get`/`io.set`/`io.copy`) and `glb`. A non-empty
///   root confines their paths to it; see [`Policy::files_dir`].
/// - `clipboard`: `io.copy` (which, living in `io`, also needs `files`), and
///   ctrl+V pasting into the cart (`drop`).
/// - `console`: the backtick console, console commands, and the editor overlays
///   (which also need `load_apps`).
/// - `load_apps`: `sub`, `over`, and `quit(n>0)` (which loads another cart or the
///   boot app). `init()` (a cart reloading itself) stays.
/// - `audio`: the sound natives exist but send nowhere; `mic` and `midi` do nothing.
/// - `host_messages`: the `host` table.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
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

impl Policy {
    /// The folder a cart's `io.get`/`io.set`/`glb` paths resolve against, given
    /// the folder its bundle was loaded from. `None`: no file access at all.
    ///
    /// `files: Some("")` (what [`Policy::open`] uses) is today's behaviour: the
    /// bundle's own folder, or nothing for a cart with no folder (a packed one).
    /// Any other root replaces the bundle folder. Either way the path a cart
    /// passes is then checked by `file_util`'s scrubbed readers and writers,
    /// which refuse absolute paths, `..` and drive prefixes, so it can't leave
    /// this folder (a symlink inside it is still followed).
    pub fn files_dir(&self, bundle_dir: Option<&str>) -> Option<String> {
        match &self.files {
            None => None,
            Some(root) if root.as_os_str().is_empty() => bundle_dir.map(str::to_string),
            Some(root) => Some(root.to_string_lossy().into_owned()),
        }
    }
}

/// The most messages either direction of the host channel holds. Past it the
/// oldest is dropped, so a side that never drains can't grow the other's memory.
pub const HOST_QUEUE_CAP: usize = 1024;

/// Host → cart messages waiting for the cart's `host.recv()`. Shared between
/// [`Core`](crate::root::Core) and the Lua threads of its cart's bundles, so a
/// `recv` is a lock and a pop, with no trip through the main thread. It lives on
/// the `Core`, not the VM, so a message posted before the cart has loaded (or
/// while it reloads) waits for it.
pub type HostInbox = Arc<Mutex<VecDeque<String>>>;

/// Push onto a host-channel queue, dropping the oldest past [`HOST_QUEUE_CAP`].
pub fn push_capped(q: &mut VecDeque<String>, msg: String) {
    while q.len() >= HOST_QUEUE_CAP {
        q.pop_front();
    }
    q.push_back(msg);
}

/// The main-thread end of the host channel, on [`Core`](crate::root::Core).
#[derive(Default)]
pub struct HostLink {
    /// Host → cart (see [`HostInbox`]).
    pub(crate) inbox: HostInbox,
    /// Cart → host: what the cart `host.send`'d, until `Core::host_take`.
    pub(crate) outbox: VecDeque<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_dir_follows_policy() {
        let open = Policy::open();
        assert_eq!(open.files_dir(Some("apps/game")), Some("apps/game".into()));
        assert_eq!(open.files_dir(None), None);

        let sandbox = Policy::sandbox();
        assert_eq!(sandbox.files_dir(Some("apps/game")), None);

        let rooted = Policy {
            files: Some(PathBuf::from("saves/cart1")),
            ..Policy::sandbox()
        };
        assert_eq!(rooted.files_dir(Some("apps/game")), Some("saves/cart1".into()));
        assert_eq!(rooted.files_dir(None), Some("saves/cart1".into()));
    }

    #[test]
    fn rooted_paths_cannot_escape() {
        let rooted = Policy {
            files: Some(PathBuf::from("test/target")),
            ..Policy::sandbox()
        };
        let dir = rooted.files_dir(None).unwrap();
        for bad in ["../Cargo.toml", "/etc/passwd", "a/../../Cargo.toml", ""] {
            assert!(
                matches!(
                    crate::file_util::get_file_string_scrubbed(&dir, bad),
                    Err(crate::error::P64Error::PermPathTraversal)
                ),
                "{bad} escaped"
            );
            assert!(matches!(
                crate::file_util::write_file_string_scrubbed(&dir, bad, "x"),
                Err(crate::error::P64Error::PermPathTraversal)
            ));
        }
    }

    #[test]
    fn queues_drop_oldest_past_cap() {
        let mut q = VecDeque::new();
        for i in 0..HOST_QUEUE_CAP + 5 {
            push_capped(&mut q, i.to_string());
        }
        assert_eq!(q.len(), HOST_QUEUE_CAP);
        assert_eq!(q.front().map(String::as_str), Some("5"));
    }

    #[test]
    fn policy_round_trips_and_defaults() {
        let p = Policy::sandbox();
        let s = serde_json::to_string(&p).unwrap();
        assert_eq!(serde_json::from_str::<Policy>(&s).unwrap(), p);

        // An Init from a main that predates the field still parses, as open.
        use crate::worker_protocol::HostToVm;
        let old = r#"{"Init":{"bundle_id":0,"width":4,"height":4}}"#;
        match serde_json::from_str::<HostToVm>(old).unwrap() {
            HostToVm::Init { policy, .. } => assert_eq!(policy, Policy::open()),
            other => panic!("{:?}", other),
        }
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

//! wasm web-worker entry points (WASM.md §4c).
//!
//! A worker is a separate wasm instance with its own memory and thread, so the
//! silt VM lives entirely here — it never crosses a thread boundary, which is
//! why `!Send`/gc-arena is a non-issue. The main thread drives us with
//! `HostToVm` messages over `postMessage`; we reply with `VmToHost`s.
//!
//! The worker keeps its OWN local `MainPacket` channel: the Lua natives send
//! `MainCommand` exactly as they do on native, and after each dispatched message
//! we drain that channel and translate (`main_command_to_host`) into the
//! `VmToHost`s posted back. No sink abstraction, no changes to the natives.
#![cfg(target_arch = "wasm32")]

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, sync_channel, Receiver, SyncSender};

use image::RgbaImage;
use parking_lot::Mutex;
use silt_lua::{Compiler, Lua};
use wasm_bindgen::prelude::*;

use crate::command::{init_lua_sys, main_command_to_host};
use crate::error::P64Error;
use crate::gui::{Gui, GuiMorsel};
use crate::log::{LogType, Loggy};
use crate::lua_define::{handle_lua_talk, LuaContext, LuaResponse, LuaTalk, MainPacket};
use crate::lua_img::LuaImg;
use crate::pad::Pad;
use crate::pool::{LocalPool, SharedPool};
use crate::types::Script;
use crate::world::{TileCommand, TileResponse};
use crate::worker_protocol::{control_state_from_wire, HostToVm, VmToHost};

/// Everything the worker's VM needs, held for its lifetime. Non-`'gc` state
/// (`ctx`, `shared`, the channels) lives here; the `'gc` VM state lives in the
/// arena root (`lua`) and is reached by re-`enter`ing per message.
struct WorkerVm {
    lua: Lua,
    ctx: LuaContext,
    shared: SharedPool,
    /// VM→host commands land here (the natives' pitcher target); drained per msg.
    catcher: Receiver<MainPacket>,
    /// Log lines from the VM; drained to the console per msg.
    loggy_rx: Receiver<(LogType, String)>,
    /// Held so the world_sender the natives hold doesn't disconnect (world runs
    /// on the host later; unused here for now).
    _world_rx: Receiver<(TileCommand, SyncSender<TileResponse>)>,
}

thread_local! {
    static WORKER_VM: RefCell<Option<WorkerVm>> = RefCell::new(None);
}

/// Called once when the worker's wasm module finishes initialising.
#[wasm_bindgen]
pub fn worker_init() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(::log::Level::Info);
    web_sys::console::log_1(&"[petrichor worker] wasm initialised".into());
}

/// Handle one message from the main thread. `msg` is a structured-clone of a
/// `HostToVm`; returns a JS array of `VmToHost` (may be empty). The main thread
/// posts each element onward.
#[wasm_bindgen]
pub fn worker_receive(msg: JsValue) -> JsValue {
    let parsed: HostToVm = match serde_wasm_bindgen::from_value(msg) {
        Ok(v) => v,
        Err(e) => {
            web_sys::console::error_1(
                &format!("[petrichor worker] undecodable message: {:?}", e).into(),
            );
            return empty_array();
        }
    };

    let out: Vec<VmToHost> = WORKER_VM.with(|cell| {
        let mut slot = cell.borrow_mut();
        match parsed {
            HostToVm::Init {
                bundle_id,
                width,
                height,
            } => {
                match build_worker_vm(bundle_id, width, height) {
                    Ok(vm) => {
                        *slot = Some(vm);
                        web_sys::console::log_1(
                            &format!("[petrichor worker] VM built (bundle {})", bundle_id).into(),
                        );
                        vec![]
                    }
                    Err(e) => {
                        web_sys::console::error_1(
                            &format!("[petrichor worker] VM build failed: {}", e).into(),
                        );
                        vec![]
                    }
                }
            }
            other => match slot.as_mut() {
                Some(worker) => dispatch(worker, other),
                None => {
                    web_sys::console::error_1(
                        &"[petrichor worker] message before Init — VM not built".into(),
                    );
                    vec![]
                }
            },
        }
    });

    serde_wasm_bindgen::to_value(&out).unwrap_or_else(|_| empty_array())
}

fn empty_array() -> JsValue {
    js_sys::Array::new().into()
}

/// Translate one non-Init `HostToVm` into a `LuaTalk`, dispatch it through the
/// shared handler, then drain the local channels and return the `VmToHost`s.
fn dispatch(worker: &mut WorkerVm, msg: HostToVm) -> Vec<VmToHost> {
    // Split the borrow so `lua.enter` (which needs &mut lua) can coexist with
    // the closure's &mut ctx / &shared.
    let WorkerVm {
        lua,
        ctx,
        shared,
        catcher,
        loggy_rx,
        ..
    } = worker;

    // Kept alive through the enter below so Load's reply send doesn't hit a
    // closed channel (the reply itself is unused on wasm — it returns via
    // postMessage). Must outlive `talk`, hence a fn-scope binding.
    let mut _load_rx = None;
    let talk: Option<LuaTalk> = match msg {
        HostToVm::Loop { keys, analog } => {
            Some(LuaTalk::Loop(control_state_from_wire(&keys, &analog)))
        }
        HostToVm::Load { name, content } => {
            let (tx, rx) = sync_channel::<LuaResponse>(1);
            _load_rx = Some(rx);
            Some(LuaTalk::Load(Box::new(Script { name, content }), tx))
        }
        HostToVm::Resize(w, h) => Some(LuaTalk::Resize(w, h)),
        HostToVm::Drop(s) => Some(LuaTalk::Drop(s)),
        HostToVm::Init { .. } => None, // handled by the caller
    };

    if let Some(talk) = talk {
        // `enter` wants an FnMut; moving `talk` into the closure would make it
        // FnOnce, so hold it in an Option and take it (mutating a capture is fine).
        let mut talk_holder = Some(talk);
        let res = lua.enter(|vm, mc| {
            let mut local_pool = LocalPool::new();
            let t = talk_holder.take().expect("dispatch enter ran more than once");
            handle_lua_talk(t, vm, mc, ctx, &mut local_pool, shared)
        });
        if let Err(e) = res {
            web_sys::console::error_1(&format!("[petrichor worker] dispatch error: {}", e).into());
        }
    }

    // Log lines → console.
    while let Ok((_kind, line)) = loggy_rx.try_recv() {
        web_sys::console::log_1(&format!("[vm] {}", line).into());
    }

    // VM→host commands → serializable messages for the main thread.
    let mut out = Vec::new();
    while let Ok((_bundle, cmd)) = catcher.try_recv() {
        if let Some(v) = main_command_to_host(cmd) {
            out.push(v);
        }
    }
    out
}

/// Build the VM the way `LuaCore::start` does, but with worker-local plumbing
/// (its own channels, self-provisioned resources, no InitBack — the worker owns
/// the gui/sky pixels and posts SetImg instead).
fn build_worker_vm(bundle_id: u8, width: u32, height: u32) -> Result<WorkerVm, P64Error> {
    let size = [width.max(1), height.max(1)];

    // Resources: the font atlas loads from the embedded fallback on wasm.
    let mut res_loggy = Loggy::new();
    let letters_img = Gui::letter_init(&mut res_loggy);
    let morsel = GuiMorsel::new(letters_img, size);
    let letters = morsel.letters.clone(); // Arc<RgbaImage>
    let gui_handle = Rc::new(RefCell::new(morsel));

    // Worker-local channels; receivers stay in WorkerVm.
    let (pitcher, catcher) = channel::<MainPacket>();
    let (loggy_tx, loggy_rx) = channel::<(LogType, String)>();
    let (world_tx, world_rx) = channel::<(TileCommand, SyncSender<TileResponse>)>();
    let ent_counter = Rc::new(Mutex::new(2u64));
    let pads = Rc::new(RefCell::new(Pad::new()));
    let shared = SharedPool::new();

    let main_im = RgbaImage::new(size[0], size[1]);
    let sky_im = RgbaImage::new(size[0], size[1]);

    let mut lua = Lua::new_with_standard();

    // Setup runs inside one enter; mutexes/compiler/scripts are created here so
    // they're closure-locals (movable into ctx), not moves out of an FnMut
    // capture. Senders are cloned in.
    let ctx = lua.enter::<_, Result<LuaContext, P64Error>>(|vm, mc| {
        let mut compiler = Compiler::new();
        let keys_mutex = Rc::new(RefCell::new([false; 256]));
        let diff_keys_mutex = Rc::new(RefCell::new([false; 256]));
        let mice_mutex = Rc::new(RefCell::new([0f32; 13]));

        let main_rast = LuaImg::new(bundle_id, main_im.clone(), size[0], size[1], letters.clone());
        let sky_rast = LuaImg::new(bundle_id, sky_im.clone(), size[0], size[1], letters.clone());
        let (main_val, _main_ref) = vm.create_userdata_tuple(mc, main_rast);
        let (sky_val, _sky_ref) = vm.create_userdata_tuple(mc, sky_rast);
        {
            let mut g = vm.globals.borrow_mut(mc);
            g.set("gui", main_val);
            g.set("sky", sky_val);
        }
        // No InitBack: the main thread can't hold Gc refs into this worker's
        // arena. The worker owns the pixels and posts SetImg when they change.

        init_lua_sys(
            vm,
            mc,
            bundle_id,
            pitcher.clone(),
            world_tx.clone(),
            Rc::clone(&gui_handle),
            Rc::clone(&keys_mutex),
            Rc::clone(&diff_keys_mutex),
            Rc::clone(&mice_mutex),
            Rc::clone(&pads),
            Rc::clone(&ent_counter),
            loggy_tx.clone(),
            LocalPool::new(),
        )?;

        let main_fn = vm.load_fn(mc, &mut compiler, Some("main_fn"), "main() loop()")?;
        let loop_fn = vm.load_fn(mc, &mut compiler, Some("loop_fn"), "loop()")?;
        let draw_fn = vm.load_fn(mc, &mut compiler, Some("draw_fn"), "draw()")?;
        let drop_fn = vm.load_fn(mc, &mut compiler, Some("drop_fn"), "drop()")?;

        Ok(LuaContext {
            bundle_id,
            compiler,
            scripts: Vec::new(),
            loggy: loggy_tx.clone(),
            main_fn,
            loop_fn,
            draw_fn,
            drop_fn,
            keys_mutex,
            diff_keys_mutex,
            mice_mutex,
            async_sender: pitcher.clone(),
        })
    })?;

    Ok(WorkerVm {
        lua,
        ctx,
        shared,
        catcher,
        loggy_rx,
        _world_rx: world_rx,
    })
}

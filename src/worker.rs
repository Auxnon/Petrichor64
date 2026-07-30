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
use crate::lua_ent::LuaEnt;
use crate::lua_img::LuaImg;
use crate::pad::Pad;
use crate::pool::{LocalPool, SharedPool};
use crate::types::Script;
use crate::world::{TileCommand, TileResponse, WorldInstance};
use crate::worker_protocol::{control_state_from_wire, ChunkWire, EntXform, HostToVm, VmToHost};

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
    /// Tile commands from the VM's `tile()` calls land here; drained + applied
    /// to the worker's own world data each frame (Option A: the VM owns the world
    /// so bulk terrain edits stay local and sync as whole chunks).
    world_rx: Receiver<(TileCommand, SyncSender<TileResponse>)>,
    /// Sound commands from the VM's note/instr/smpl calls. No AudioContext in a
    /// worker, so these are drained + forwarded to the main thread's cpal stream.
    #[cfg(feature = "audio")]
    audio_rx: Receiver<crate::sound::SoundCommand>,
    /// The worker's world tile data (replaces the per-bundle world thread).
    world_layer: crate::tile::Layer,
    world_instance: WorldInstance,
    /// Live handles to spawned entities (Weak refs into the VM's userdata). Read
    /// each frame to stream transforms to the main thread; dropped entries are
    /// reaped when their upgrade fails.
    entities: Vec<silt_lua::userdata::UserDataWrapper>,
    /// Last transform streamed per entity id, so we only send the ones that
    /// actually changed (dirty-only streaming). A static entity costs nothing
    /// after its first frame; main keeps the last value it received.
    last_xforms: std::collections::HashMap<u64, EntXform>,
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
            return empty_envelope();
        }
    };

    let (out, ent_bytes): (Vec<VmToHost>, Vec<u8>) = WORKER_VM.with(|cell| {
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
                        (vec![], Vec::new())
                    }
                    Err(e) => {
                        web_sys::console::error_1(
                            &format!("[petrichor worker] VM build failed: {}", e).into(),
                        );
                        (vec![], Vec::new())
                    }
                }
            }
            other => match slot.as_mut() {
                Some(worker) => dispatch(worker, other),
                None => {
                    web_sys::console::error_1(
                        &"[petrichor worker] message before Init — VM not built".into(),
                    );
                    (vec![], Vec::new())
                }
            },
        }
    });

    // Sound commands leave in their own field, already in the worklet's MessagePack
    // wire format. They have a private lane to the audio thread (see web/worker.js
    // and `WebOut::open_command_lane`) so a note doesn't wait for the main thread to
    // receive it, apply it on one frame and forward it on the next — up to two
    // frames of latency in front of a synth that renders in 2.7ms blocks. Encoded
    // here rather than in JS because the format is the synth crate's, and because
    // these can carry PCM: as a serde-wasm-bindgen array that would be one boxed
    // JS number per sample.
    #[cfg(feature = "audio")]
    let (out, sounds) = {
        let sounds = js_sys::Array::new();
        let mut rest = Vec::with_capacity(out.len());
        for m in out {
            match m {
                VmToHost::Sound(cmd) => {
                    if let Some(bytes) = crate::sound::encode_command(&cmd) {
                        sounds.push(&js_sys::Uint8Array::from(bytes.as_slice()).into());
                    }
                }
                other => rest.push(other),
            }
        }
        (rest, sounds)
    };

    // Envelope: `{ msgs, ents?, sounds? }`. Structured messages go via serde; the
    // per-frame entity buffer rides as a Uint8Array the JS glue transfers
    // zero-copy (no SharedArrayBuffer / isolation headers required).
    let obj = js_sys::Object::new();
    let msgs = serde_wasm_bindgen::to_value(&out).unwrap_or_else(|_| js_sys::Array::new().into());
    let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("msgs"), &msgs);
    #[cfg(feature = "audio")]
    if sounds.length() > 0 {
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("sounds"), &sounds);
    }
    if !ent_bytes.is_empty() {
        let arr = js_sys::Uint8Array::from(ent_bytes.as_slice());
        let _ = js_sys::Reflect::set(&obj, &JsValue::from_str("ents"), &arr);
    }
    obj.into()
}

/// An envelope with no messages and no entity buffer — the shape main expects
/// even on the error/no-op paths.
fn empty_envelope() -> JsValue {
    let obj = js_sys::Object::new();
    let _ = js_sys::Reflect::set(
        &obj,
        &JsValue::from_str("msgs"),
        &js_sys::Array::new().into(),
    );
    obj.into()
}

/// Translate one non-Init `HostToVm` into a `LuaTalk`, dispatch it through the
/// shared handler, then drain the local channels. Returns the structured
/// `VmToHost` messages plus the packed per-frame entity buffer (transferred
/// zero-copy by the caller).
fn dispatch(worker: &mut WorkerVm, msg: HostToVm) -> (Vec<VmToHost>, Vec<u8>) {
    // Split the borrow so `lua.enter` (which needs &mut lua) can coexist with
    // the closure's &mut ctx / &shared.
    let WorkerVm {
        lua,
        ctx,
        shared,
        catcher,
        loggy_rx,
        world_rx,
        #[cfg(feature = "audio")]
        audio_rx,
        world_layer,
        world_instance,
        entities,
        last_xforms,
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
        HostToVm::Main => Some(LuaTalk::Main),
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
        match cmd {
            // Keep the live entity handle (Weak into the VM's userdata) so we can
            // stream its transform each frame, and forward the initial Spawn.
            crate::command::MainCommmand::Spawn(wrapper) => {
                if let Ok(lent) = wrapper.downcast_ref::<LuaEnt, _, _>(|l| Ok(l.clone())) {
                    out.push(VmToHost::Spawn(lent));
                }
                entities.push(wrapper);
            }
            other => {
                if let Some(v) = main_command_to_host(other) {
                    out.push(v);
                }
            }
        }
    }

    // Apply this frame's tile() calls to the worker's own world data, then sync
    // any dirty chunks to main for GPU meshing (Option A — bulk edits stayed
    // local; we ship whole chunks, not per-tile messages).
    while let Ok((cmd, _reply)) = world_rx.try_recv() {
        match cmd {
            TileCommand::Set(tiles) => {
                if let Some((name, v)) = tiles.into_iter().next() {
                    let name = name.to_lowercase();
                    // Map the tile texture if new, and tell main so its local
                    // mapper can resolve the cell's int back to an atlas uv.
                    if let Some(index) = world_instance.ensure_tex(&name) {
                        out.push(VmToHost::MapTex {
                            name: name.clone(),
                            index,
                        });
                    }
                    // v = ivec4(meta/rot, x, y, z) — see set_tile in command.rs.
                    world_layer
                        .set_tile(world_instance, &name, v.x as u8, v.y, v.z, v.w);
                }
            }
            TileCommand::Clear() | TileCommand::Destroy() => {
                world_layer.destroy_it_all();
            }
            _ => {}
        }
    }

    // Forward the VM's sound commands to the main thread (which owns the cpal
    // stream — a worker has no AudioContext).
    #[cfg(feature = "audio")]
    while let Ok(cmd) = audio_rx.try_recv() {
        out.push(VmToHost::Sound(cmd));
    }

    let dirty = world_layer.get_dirty();
    if !dirty.is_empty() {
        out.push(VmToHost::WorldSync {
            chunks: dirty.iter().map(ChunkWire::from_chunk).collect(),
            dropped: false,
        });
    }

    // Stream live transforms of surviving entities; reap dropped ones (their
    // Weak fails to upgrade). This is the no-SAB movement channel: pack straight
    // into a flat LE buffer that the caller transfers zero-copy, rather than a
    // serde array of N objects (see worker_protocol::ENT_STRIDE).
    //
    // Dirty-only: an entity is packed only if new or its transform changed since
    // last frame (exact compare — same Lua value → identical bytes). A scene of
    // mostly-static entities streams almost nothing; main keeps the last value.
    let mut ent_bytes = Vec::new();
    let mut alive: std::collections::HashSet<u64> = std::collections::HashSet::new();
    let mut removed: std::collections::HashSet<u64> = std::collections::HashSet::new();
    entities.retain(|w| {
        match w.downcast_ref::<LuaEnt, _, _>(|l| {
            Ok((
                l.is_dead(),
                EntXform {
                    id: l.get_id(),
                    x: l.x as f32,
                    y: l.y as f32,
                    z: l.z as f32,
                    rx: l.rot_x as f32,
                    ry: l.rot_y as f32,
                    rz: l.rot_z as f32,
                    scale: l.scale as f32,
                },
            ))
        }) {
            Ok((true, xf)) => {
                // Killed this frame (DEAD flag): report removal, stop tracking.
                removed.insert(xf.id);
                false
            }
            Ok((false, xf)) => {
                alive.insert(xf.id);
                if last_xforms.get(&xf.id) != Some(&xf) {
                    xf.write_le(&mut ent_bytes);
                    last_xforms.insert(xf.id, xf);
                }
                true
            }
            // Weak upgrade failed → the VM GC'd it; caught by the diff below.
            Err(_) => false,
        }
    });
    // Any id we streamed before but didn't see alive this frame is gone (killed
    // or GC-reaped). Report it and drop its cache entry so ids can't leak.
    for id in last_xforms.keys() {
        if !alive.contains(id) {
            removed.insert(*id);
        }
    }
    last_xforms.retain(|id, _| alive.contains(id));
    if !removed.is_empty() {
        out.push(VmToHost::EntRemove(removed.into_iter().collect()));
    }
    (out, ent_bytes)
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
    #[cfg(feature = "audio")]
    let (audio_tx, audio_rx) = channel::<crate::sound::SoundCommand>();
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
            #[cfg(feature = "audio")]
            audio_tx.clone(),
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
        world_rx,
        #[cfg(feature = "audio")]
        audio_rx,
        world_layer: crate::tile::Layer::new(),
        world_instance: WorldInstance::new(bundle_id),
        entities: Vec::new(),
        last_xforms: std::collections::HashMap::new(),
    })
}

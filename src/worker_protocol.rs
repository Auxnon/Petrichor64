//! Serializable message protocol for the wasm web-worker VM boundary.
//!
//! On native the Lua VM runs on its own thread and talks to the main thread via
//! the mpsc `LuaTalk` (host→VM) and `MainCommmand` (VM→host) enums directly.
//! Those enums carry reply channels (`SyncSender`) and GPU/Rust-native payloads
//! that can't cross a web worker's `postMessage`, so on wasm we translate to and
//! from the plain, serde-serializable messages defined here.
//!
//! This first cut covers the fire-and-forget subset that needs no reply
//! (see WASM.md §2): `Loop`/`Resize`/`Drop` host→VM, and `Spawn`/`SetImg`/`Cam`/
//! `Globals`/`LoopComplete` VM→host. Request/response calls (func, get_img,
//! get_global, read/write, die) are added with the read-back phase.
//!
//! The module compiles on every target (it's pure data + serde) so the contract
//! is type-checked everywhere; it is only *used* by the wasm worker bridge.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};

use crate::lua_ent::LuaEnt;
use crate::types::{ControlState, ValueMap};

/// Main thread → VM worker.
#[derive(Serialize, Deserialize, Debug)]
pub enum HostToVm {
    /// Build the VM (once, before any other message): create the arena, register
    /// natives, load the main/loop/draw/drop entry points. `width`/`height` size
    /// the gui/sky rasters.
    Init {
        bundle_id: u8,
        width: u32,
        height: u32,
    },
    /// Compile + run a script into the VM (e.g. a bundle's main.lua).
    Load { name: String, content: String },
    /// Call the app's `main()` once (after scripts are loaded).
    Main,
    /// Run one game loop with this input snapshot. `keys` is one byte per key
    /// (0/1) — a Vec, not `[u8; 256]`, because serde's derive only covers arrays
    /// up to 32 elements. `analog` is the mouse/scroll/cursor floats.
    Loop { keys: Vec<u8>, analog: Vec<f32> },
    /// Viewport resized.
    Resize(u32, u32),
    /// A file/app was dropped or requested for load-in-place.
    Drop(String),
}

/// VM worker → main thread.
#[derive(Serialize, Deserialize, Debug)]
pub enum VmToHost {
    /// Create a render entity from this VM-owned entity (the VM already minted
    /// the id and owns the handle; this just tells the GPU side to build it).
    Spawn(LuaEnt),
    /// Upload/replace a named texture's pixels (RGBA8, row-major).
    SetImg {
        name: String,
        w: u32,
        h: u32,
        px: Vec<u8>,
    },
    /// Move/rotate the camera. `pos` is xyz, `rot` is the simple yaw/pitch pair.
    Cam {
        pos: Option<[f32; 3]>,
        rot: Option<[f32; 2]>,
    },
    /// Set engine globals (screen effects, window props, …).
    Globals(Vec<(String, ValueWire)>),
    /// The VM finished a `Loop`; `gui`/`sky` flag which rasters changed so the
    /// host re-uploads them.
    LoopComplete { gui: bool, sky: bool },
    /// Per-frame live transforms of the worker's entities (the VM mutates them;
    /// this mirrors the changes to the main-thread render copies). Since the VM
    /// lives in a separate wasm instance there's no shared memory — this is the
    /// no-SAB movement channel. (Ultra tier will replace it with a
    /// SharedArrayBuffer; a transferable Float32Array is the drop-in optimisation.)
    EntUpdate(Vec<EntXform>),
    /// Dirty world chunks the worker's VM built (bulk terrain edits stay local
    /// to the VM and sync as whole chunks — the "phantom chunk" batching). Main
    /// rebuilds the GPU chunk models via World::process_sync.
    WorldSync { chunks: Vec<ChunkWire>, dropped: bool },
    /// Sync a tile-texture name→index mapping so main's local mapper can resolve
    /// a chunk cell's int back to an atlas uv.
    MapTex { name: String, index: u32 },
    /// A runtime/async error to surface in the engine console.
    Error(String),
}

/// Serializable form of a world [`Chunk`] (its `cells` are a fixed 32³ array,
/// which serde can't derive; split into parallel Vecs on the wire). This is the
/// correct-but-heavy version — a transferable ArrayBuffer / sparse encoding is
/// the optimisation for frequent multi-chunk edits.
#[derive(Serialize, Deserialize, Debug)]
pub struct ChunkWire {
    pub key: String,
    pub pos: [i32; 3],
    pub types: Vec<u32>,
    pub meta: Vec<u8>,
}

impl ChunkWire {
    pub fn from_chunk(c: &crate::tile::Chunk) -> Self {
        ChunkWire {
            key: c.key.clone(),
            pos: [c.pos.x, c.pos.y, c.pos.z],
            types: c.cells.iter().map(|(t, _)| *t).collect(),
            meta: c.cells.iter().map(|(_, m)| *m).collect(),
        }
    }
    pub fn into_chunk(self) -> crate::tile::Chunk {
        let mut chunk = crate::tile::Chunk::new(self.key, self.pos[0], self.pos[1], self.pos[2]);
        for (i, (t, m)) in self.types.iter().zip(self.meta.iter()).enumerate() {
            if i < chunk.cells.len() {
                chunk.cells[i] = (*t, *m);
            }
        }
        chunk.dirty = true;
        chunk
    }
}

/// One entity's transform, streamed each frame (see [`VmToHost::EntUpdate`]).
#[derive(Serialize, Deserialize, Debug)]
pub struct EntXform {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rx: f32,
    pub ry: f32,
    pub rz: f32,
    pub scale: f32,
}

/// Serializable mirror of [`ValueMap`] (which isn't itself `Serialize`).
#[derive(Serialize, Deserialize, Debug)]
pub enum ValueWire {
    Str(String),
    Int(i32),
    Float(f32),
    Bool(bool),
    Array(Vec<ValueWire>),
    Null,
}

impl HostToVm {
    /// Pack a `ControlState` into a `Loop` message (bools → 0/1 bytes).
    pub fn loop_from(cs: &ControlState) -> Self {
        HostToVm::Loop {
            keys: cs.0.iter().map(|b| *b as u8).collect(),
            analog: cs.1.to_vec(),
        }
    }
}

/// Rebuild a `ControlState` from the wire representation (0/1 bytes → bools).
/// Extra/short input is clamped to the fixed engine sizes.
pub fn control_state_from_wire(keys: &[u8], analog: &[f32]) -> ControlState {
    let mut k = [false; 256];
    for (slot, b) in k.iter_mut().zip(keys.iter()) {
        *slot = *b != 0;
    }
    let mut a = [0f32; 11];
    for (slot, v) in a.iter_mut().zip(analog.iter()) {
        *slot = *v;
    }
    ControlState(k, a)
}

impl From<&ValueMap> for ValueWire {
    fn from(v: &ValueMap) -> Self {
        match v {
            ValueMap::String(s) => ValueWire::Str(s.clone()),
            ValueMap::Integer(i) => ValueWire::Int(*i),
            ValueMap::Float(f) => ValueWire::Float(*f),
            ValueMap::Bool(b) => ValueWire::Bool(*b),
            ValueMap::Array(a) => ValueWire::Array(a.iter().map(ValueWire::from).collect()),
            ValueMap::Null() => ValueWire::Null,
        }
    }
}

impl From<&ValueWire> for ValueMap {
    fn from(v: &ValueWire) -> Self {
        match v {
            ValueWire::Str(s) => ValueMap::String(s.clone()),
            ValueWire::Int(i) => ValueMap::Integer(*i),
            ValueWire::Float(f) => ValueMap::Float(*f),
            ValueWire::Bool(b) => ValueMap::Bool(*b),
            ValueWire::Array(a) => ValueMap::Array(a.iter().map(ValueMap::from).collect()),
            ValueWire::Null => ValueMap::Null(),
        }
    }
}

//! wasm web-worker entry points (WASM.md §4c).
//!
//! A worker is a separate wasm instance with its own memory and thread, so the
//! silt VM will live entirely here — never crossing a thread boundary. This
//! first cut proves the transport: `worker_receive` deserializes a `HostToVm`
//! from the main thread and returns a `VmToHost` (via serde-wasm-bindgen /
//! structured clone). The VM itself is wired in the next iteration; for now the
//! thread-local holder is empty and messages are logged + acknowledged.
#![cfg(target_arch = "wasm32")]

use wasm_bindgen::prelude::*;

use crate::worker_protocol::{HostToVm, VmToHost};

/// Called once when the worker's wasm module finishes initialising.
#[wasm_bindgen]
pub fn worker_init() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(::log::Level::Info);
    web_sys::console::log_1(&"[petrichor worker] wasm initialised".into());
}

/// Handle one message from the main thread. `msg` is a structured-clone of a
/// `HostToVm`; the return is a `VmToHost` (or `JsValue::NULL` when there's
/// nothing to send back). The main thread posts the result onward.
#[wasm_bindgen]
pub fn worker_receive(msg: JsValue) -> JsValue {
    let parsed: Result<HostToVm, _> = serde_wasm_bindgen::from_value(msg);
    let reply: Option<VmToHost> = match parsed {
        Ok(HostToVm::Loop { keys, analog }) => {
            web_sys::console::log_1(
                &format!(
                    "[petrichor worker] Loop (keys={}, analog={})",
                    keys.len(),
                    analog.len()
                )
                .into(),
            );
            // Transport check: acknowledge the frame. Once the VM is wired this
            // becomes the drained + translated VmToHost stream for the frame.
            Some(VmToHost::LoopComplete {
                gui: true,
                sky: true,
            })
        }
        Ok(HostToVm::Resize(w, h)) => {
            web_sys::console::log_1(&format!("[petrichor worker] Resize {}x{}", w, h).into());
            None
        }
        Ok(HostToVm::Drop(s)) => {
            web_sys::console::log_1(&format!("[petrichor worker] Drop {}", s).into());
            None
        }
        Err(e) => {
            web_sys::console::error_1(
                &format!("[petrichor worker] undecodable message: {:?}", e).into(),
            );
            None
        }
    };

    match reply {
        Some(v) => serde_wasm_bindgen::to_value(&v).unwrap_or(JsValue::NULL),
        None => JsValue::NULL,
    }
}

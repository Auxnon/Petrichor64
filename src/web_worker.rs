//! Main-thread handle to the VM web worker (WASM.md §4d).
//!
//! The engine (main thread) spawns the worker, posts `HostToVm` messages
//! (Init/Load/Loop/…), and collects the `VmToHost` messages the worker posts
//! back (Spawn/SetImg/Cam/LoopComplete/…) into an inbox the render loop drains.
#![cfg(all(feature = "headed", target_arch = "wasm32"))]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;

use crate::worker_protocol::{HostToVm, VmToHost};

pub struct WorkerHandle {
    worker: web_sys::Worker,
    /// True once the worker posted `{kind:"worker-ready"}` (its wasm is up).
    ready: Rc<RefCell<bool>>,
    /// VmToHost messages received from the worker, drained by the render loop.
    inbox: Rc<RefCell<VecDeque<VmToHost>>>,
    // Kept alive for the worker's lifetime; dropping it detaches the handler.
    _onmessage: Closure<dyn FnMut(web_sys::MessageEvent)>,
}

impl WorkerHandle {
    /// Spawn the module worker at `url` (e.g. "/worker.js").
    pub fn spawn(url: &str) -> Result<Self, JsValue> {
        let opts = web_sys::WorkerOptions::new();
        opts.set_type(web_sys::WorkerType::Module);
        let worker = web_sys::Worker::new_with_options(url, &opts)?;

        let ready = Rc::new(RefCell::new(false));
        let inbox = Rc::new(RefCell::new(VecDeque::new()));

        let ready_cb = ready.clone();
        let inbox_cb = inbox.clone();
        let onmessage = Closure::wrap(Box::new(move |e: web_sys::MessageEvent| {
            let data = e.data();
            let kind = js_sys::Reflect::get(&data, &JsValue::from_str("kind"))
                .ok()
                .and_then(|k| k.as_string());
            match kind.as_deref() {
                Some("worker-ready") => *ready_cb.borrow_mut() = true,
                Some("vm") => {
                    if let Ok(payload) = js_sys::Reflect::get(&data, &JsValue::from_str("payload")) {
                        match serde_wasm_bindgen::from_value::<VmToHost>(payload) {
                            Ok(v) => inbox_cb.borrow_mut().push_back(v),
                            Err(e) => web_sys::console::error_1(
                                &format!("[main] undecodable VmToHost: {:?}", e).into(),
                            ),
                        }
                    }
                }
                _ => {}
            }
        }) as Box<dyn FnMut(web_sys::MessageEvent)>);

        worker.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));

        Ok(Self {
            worker,
            ready,
            inbox,
            _onmessage: onmessage,
        })
    }

    pub fn is_ready(&self) -> bool {
        *self.ready.borrow()
    }

    /// Post a message to the worker. Serialization failures are logged, not fatal.
    pub fn post(&self, msg: &HostToVm) {
        match serde_wasm_bindgen::to_value(msg) {
            Ok(js) => {
                let _ = self.worker.post_message(&js);
            }
            Err(e) => {
                web_sys::console::error_1(&format!("[main] cannot encode HostToVm: {:?}", e).into())
            }
        }
    }

    /// Take everything the worker has posted since the last drain.
    pub fn drain(&self) -> Vec<VmToHost> {
        self.inbox.borrow_mut().drain(..).collect()
    }
}

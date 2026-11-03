//! A lost GPU, and getting it back without losing the game.
//!
//! A device can go away under a running game: a driver reset natively, the
//! browser reclaiming a backgrounded tab's WebGL context, or WebGPU's
//! `GPUDevice.lost`. Everything made on it is dead from then on, so the engine
//! used to draw nothing (a frozen purple screen on the web) or exit.
//!
//! Now a loss stops drawing, and once a device can be had again the main loop
//! builds a new `Gfx` and re-uploads everything from the CPU copies the engine
//! keeps anyway (`Core::restore_gpu`). The VM, entities and world never notice.
//!
//! How a loss arrives:
//! - native and WebGPU: wgpu's device-lost callback (`watch`);
//! - WebGL2: the canvas's `webglcontextlost`, and a new device can't be made
//!   until `webglcontextrestored` (`watch_canvas`).
//!
//! On the web the page hears about it too: `petrichor-gpu-lost` and
//! `petrichor-gpu-restored` events on `window`, for a host page that wants to
//! show something meanwhile. `CHOR_GPU_LOSE=secs` (native) loses the device on
//! purpose that long after start, for testing.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

/// The device is gone: don't draw, restore when `ready`.
static LOST: AtomicBool = AtomicBool::new(false);
/// WebGL2 only: lost, and the browser hasn't handed the context back yet.
static WAITING: AtomicBool = AtomicBool::new(false);
/// Which device is current: each `watch` gets the next. A replaced device
/// reports itself lost when it's dropped, which is no loss of ours.
static DEVICE: AtomicU32 = AtomicU32::new(0);

/// Listen for this device being lost, and stop its errors killing the app.
pub fn watch(device: &wgpu::Device) {
    let this = DEVICE.fetch_add(1, Ordering::SeqCst) + 1;
    device.set_device_lost_callback(move |reason, msg| {
        if DEVICE.load(Ordering::SeqCst) == this {
            lose(&format!("{reason:?}: {msg}"));
        }
    });
    device.on_uncaptured_error(Arc::new(|e: wgpu::Error| {
        if is_lost() {
            // What a dead device says on its way out; the restore deals with it.
            return;
        }
        #[cfg(target_arch = "wasm32")]
        ::log::error!("gpu error: {e}");
        #[cfg(not(target_arch = "wasm32"))]
        {
            crate::error_window(Box::new(e));
            std::process::exit(1);
        }
    }));
}

/// `create_buffer_init` for anything made while a game runs. On a lost
/// device it makes an empty, unmapped buffer instead: WebGPU throws on
/// `mappedAtCreation` once the device is gone, and wgpu unwraps that, killing
/// the module before a restore can run. The restore remakes these anyway.
pub trait BufferInit {
    fn buffer_init(&self, desc: &wgpu::util::BufferInitDescriptor) -> wgpu::Buffer;
}

impl BufferInit for wgpu::Device {
    fn buffer_init(&self, desc: &wgpu::util::BufferInitDescriptor) -> wgpu::Buffer {
        if is_lost() {
            return self.create_buffer(&wgpu::BufferDescriptor {
                label: desc.label,
                size: (desc.contents.len() as u64).next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT),
                usage: desc.usage,
                mapped_at_creation: false,
            });
        }
        wgpu::util::DeviceExt::create_buffer_init(self, desc)
    }
}

/// Mark the GPU lost (once), and tell the page.
pub fn lose(why: &str) {
    if !LOST.swap(true, Ordering::SeqCst) {
        ::log::warn!("gpu lost ({why}); drawing stops until it's back");
        #[cfg(not(target_arch = "wasm32"))]
        println!("gpu lost ({why})");
        #[cfg(target_arch = "wasm32")]
        dispatch("petrichor-gpu-lost");
    }
}

pub fn is_lost() -> bool {
    LOST.load(Ordering::SeqCst)
}

/// Lost, and a new device can be asked for.
pub fn ready() -> bool {
    is_lost() && !WAITING.load(Ordering::SeqCst)
}

/// A new `Gfx` is in: draw again.
pub fn restored() {
    LOST.store(false, Ordering::SeqCst);
    ::log::info!("gpu restored");
    #[cfg(not(target_arch = "wasm32"))]
    println!("gpu restored");
    #[cfg(target_arch = "wasm32")]
    dispatch("petrichor-gpu-restored");
}

/// `CHOR_GPU_LOSE=secs`: seconds after start to lose the device on purpose.
#[cfg(not(target_arch = "wasm32"))]
pub fn test_loss_at() -> Option<f32> {
    std::env::var("CHOR_GPU_LOSE").ok()?.parse().ok()
}

/// Lose `device` now, as a driver reset would. `CHOR_GPU_LOSE_SOFT` only
/// marks it lost, which tests the rebuild without the driver's teardown.
pub fn simulate(device: &wgpu::Device) {
    lose("simulated");
    if std::env::var_os("CHOR_GPU_LOSE_SOFT").is_none() {
        device.destroy();
    }
}

/// WebGL2 reports a loss on the canvas, not the device. `preventDefault` on
/// the loss is what lets the browser restore the context at all.
#[cfg(target_arch = "wasm32")]
pub fn watch_canvas(canvas: &web_sys::HtmlCanvasElement) {
    use wasm_bindgen::{closure::Closure, JsCast};
    let on_lost = Closure::<dyn FnMut(web_sys::Event)>::new(|e: web_sys::Event| {
        e.prevent_default();
        WAITING.store(true, Ordering::SeqCst);
        lose("webglcontextlost");
    });
    let on_restored = Closure::<dyn FnMut(web_sys::Event)>::new(|_: web_sys::Event| {
        ::log::info!("webgl context back; rebuilding");
        WAITING.store(false, Ordering::SeqCst);
    });
    let _ = canvas
        .add_event_listener_with_callback("webglcontextlost", on_lost.as_ref().unchecked_ref());
    let _ = canvas.add_event_listener_with_callback(
        "webglcontextrestored",
        on_restored.as_ref().unchecked_ref(),
    );
    on_lost.forget();
    on_restored.forget();
}

#[cfg(target_arch = "wasm32")]
fn dispatch(name: &str) {
    if let (Some(w), Ok(e)) = (web_sys::window(), web_sys::Event::new(name)) {
        let _ = w.dispatch_event(&e);
    }
}

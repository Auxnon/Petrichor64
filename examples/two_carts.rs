//! Two carts in one window: Petrichor64 as a library.
//!
//! ```sh
//! cargo run --example two_carts -- path/to/cart-a path/to/cart-b
//! ```
//!
//! The host (this file) owns the window, the wgpu device and the event loop.
//! Each cart is an `Instance` on that device, drawing into a texture of its
//! own; the host draws the two textures side by side. Click a cart to focus
//! it: keys, the pointer and the wheel go to the focused one only.
//!
//! The left cart runs with an open policy. The right one is sandboxed
//! (`Policy::sandbox()`): no network, files, clipboard, console or loading of
//! other carts, so its Lua sees `conn` and `io` as `nil`. With no paths given,
//! both run the engine's built-in boot app.
//!
//! `host.send("...")` from either cart is printed here; press F1 to post a
//! message to the focused cart (`host.recv()`).

use std::sync::Arc;
use std::time::{Duration, Instant};

use petrichor64::host::{Button, Cart, HostGpu, Input, Options, Policy, Request, TouchPhase};
use petrichor64::wgpu;
use petrichor64::winit::{
    self,
    application::ApplicationHandler,
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
    window::{Window, WindowId},
};
use petrichor64::Instance;

/// Gap around and between the carts, in physical pixels.
const GAP: u32 = 8;

const BLIT: &str = r#"
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;
struct Out { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
    // one triangle covering the viewport
    let p = vec2<f32>(f32((i << 1u) & 2u), f32(i & 2u));
    var o: Out;
    o.pos = vec4<f32>(p * 2.0 - 1.0, 0.0, 1.0);
    o.uv = vec2<f32>(p.x, 1.0 - p.y);
    return o;
}
@fragment fn fs(i: Out) -> @location(0) vec4<f32> {
    return textureSample(tex, samp, i.uv);
}
"#;

/// One cart and where the host shows it.
struct Slot {
    inst: Instance,
    /// x, y, w, h in the window's physical pixels
    rect: [u32; 4],
    /// The instance's texture, bound for the blit. Remade after a resize.
    bind: wgpu::BindGroup,
}

struct Host {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    gpu: HostGpu,
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    slots: Vec<Slot>,
    focus: usize,
    cursor: PhysicalPosition<f64>,
    start: Instant,
}

/// The two rects for a window of `w` x `h`.
fn rects(w: u32, h: u32) -> [[u32; 4]; 2] {
    let cw = (w.saturating_sub(GAP * 3) / 2).max(1);
    let ch = h.saturating_sub(GAP * 2).max(1);
    [[GAP, GAP, cw, ch], [GAP * 2 + cw, GAP, cw, ch]]
}

fn bind(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    view: &wgpu::TextureView,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("cart"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    })
}

impl Host {
    fn new(event_loop: &ActiveEventLoop, carts: [Cart; 2]) -> Self {
        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_title("two carts")
                        .with_inner_size(LogicalSize::new(1100, 480)),
                )
                .expect("a window"),
        );
        let size = window.inner_size();

        // The host's own wgpu setup: nothing here is Petrichor's.
        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).expect("a surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .expect("an adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("a device");
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .map(|mut c| {
                c.format = format;
                c
            })
            .expect("a surface config");
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(BLIT.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                targets: &[Some(format.into())],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());

        // What the engine is lent: the device, the queue, and the format the
        // host will sample its texture as.
        let gpu = HostGpu {
            device,
            queue,
            format,
        };
        let scale = window.scale_factor() as f32;
        let policies = [Policy::open(), Policy::sandbox()];
        let slots = rects(size.width, size.height)
            .into_iter()
            .zip(carts)
            .zip(policies)
            .map(|((rect, cart), policy)| {
                let inst = Instance::new(
                    &gpu,
                    Options {
                        cart,
                        size: (rect[2], rect[3]),
                        scale,
                        policy,
                    },
                )
                .unwrap_or_else(|e| panic!("cart failed to load: {e}"));
                let bind = bind(&gpu.device, &layout, &sampler, inst.texture());
                Slot { inst, rect, bind }
            })
            .collect();

        Self {
            window,
            surface,
            config,
            gpu,
            pipeline,
            layout,
            sampler,
            slots,
            focus: 0,
            cursor: PhysicalPosition::new(0., 0.),
            start: Instant::now(),
        }
    }

    fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.config.width = w;
        self.config.height = h;
        self.surface.configure(&self.gpu.device, &self.config);
        let scale = self.window.scale_factor() as f32;
        for (slot, rect) in self.slots.iter_mut().zip(rects(w, h)) {
            slot.rect = rect;
            slot.inst.resize((rect[2], rect[3]), scale);
            // a resized instance has a new texture
            slot.bind = bind(&self.gpu.device, &self.layout, &self.sampler, slot.inst.texture());
        }
    }

    /// The slot under the cursor, if any.
    fn hit(&self) -> Option<usize> {
        let (x, y) = (self.cursor.x as i64, self.cursor.y as i64);
        self.slots.iter().position(|s| {
            let [rx, ry, rw, rh] = s.rect.map(|v| v as i64);
            x >= rx && x < rx + rw && y >= ry && y < ry + rh
        })
    }

    fn focus(&mut self, to: usize) {
        if to != self.focus {
            self.slots[self.focus].inst.input(Input::FocusLost);
            self.focus = to;
            self.window
                .set_title(&format!("two carts: {} has focus", ["left", "right"][to]));
        }
    }

    /// To the focused cart.
    fn send(&mut self, ev: Input) {
        self.slots[self.focus].inst.input(ev);
    }

    /// Advance both carts, and deal with what they asked for.
    fn tick(&mut self, event_loop: &ActiveEventLoop) {
        let now = self.start.elapsed();
        let mut drew = false;
        for (i, slot) in self.slots.iter_mut().enumerate() {
            if slot.inst.frame(now) {
                slot.inst.render();
                drew = true;
            }
            for request in slot.inst.requests() {
                match request {
                    // Only the trusted cart may close the window; the rest
                    // (title, fullscreen, size, pointer grab, clipboard) this
                    // host declines.
                    Request::Quit if i == 0 => event_loop.exit(),
                    other => println!("cart {i} asked for {other:?}: ignored"),
                }
            }
            for msg in slot.inst.messages() {
                println!("cart {i} says: {msg}");
            }
        }
        if drew {
            self.window.request_redraw();
        }
    }

    fn draw(&mut self) {
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("host"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.02,
                            g: 0.02,
                            b: 0.03,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            for slot in &self.slots {
                let [x, y, w, h] = slot.rect;
                // a window shrunk past the gaps leaves a rect off its edge
                if x + w > self.config.width || y + h > self.config.height {
                    continue;
                }
                pass.set_viewport(x as f32, y as f32, w as f32, h as f32, 0., 1.);
                pass.set_bind_group(0, &slot.bind, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        self.gpu.queue.submit([encoder.finish()]);
        self.gpu.queue.present(frame);
    }
}

struct App {
    carts: Option<[Cart; 2]>,
    host: Option<Host>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(carts) = self.carts.take() {
            self.host = Some(Host::new(event_loop, carts));
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(host) = &mut self.host else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => host.resize(size.width, size.height),
            WindowEvent::RedrawRequested => host.draw(),
            WindowEvent::KeyboardInput { event, .. } => {
                let pressed = event.state == ElementState::Pressed;
                if let PhysicalKey::Code(code) = event.physical_key {
                    if code == KeyCode::F1 {
                        if pressed && !event.repeat {
                            let focus = host.focus;
                            host.slots[focus].inst.post("hello from the host".into());
                        }
                        return;
                    }
                    host.send(Input::Key { code, pressed });
                }
                if pressed {
                    match &event.logical_key {
                        Key::Character(c) => host.send(Input::Text(c.to_string())),
                        Key::Named(NamedKey::Space) => host.send(Input::Text(" ".into())),
                        _ => {}
                    }
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                let s = mods.state();
                host.send(Input::Modifiers {
                    alt: s.alt_key(),
                    ctrl: s.control_key(),
                    shift: s.shift_key(),
                    logo: s.super_key(),
                });
            }
            WindowEvent::CursorMoved { position, .. } => {
                host.cursor = position;
                // in the focused cart's own pixels: the host subtracts its rect
                let [rx, ry, ..] = host.slots[host.focus].rect;
                host.send(Input::PointerMoved {
                    x: position.x as f32 - rx as f32,
                    y: position.y as f32 - ry as f32,
                });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let pressed = state == ElementState::Pressed;
                if pressed {
                    if let Some(to) = host.hit() {
                        host.focus(to);
                        let [rx, ry, ..] = host.slots[to].rect;
                        host.send(Input::PointerMoved {
                            x: host.cursor.x as f32 - rx as f32,
                            y: host.cursor.y as f32 - ry as f32,
                        });
                    }
                }
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    MouseButton::Middle => Button::Middle,
                    MouseButton::Forward => Button::Forward,
                    _ => return,
                };
                host.send(Input::Button { button, pressed });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (x, y),
                    MouseScrollDelta::PixelDelta(p) => (p.x as f32 / 120., p.y as f32 / 120.),
                };
                host.send(Input::Wheel { x, y });
            }
            WindowEvent::Touch(t) => {
                let [rx, ry, ..] = host.slots[host.focus].rect;
                host.send(Input::Touch {
                    id: t.id,
                    phase: match t.phase {
                        winit::event::TouchPhase::Started => TouchPhase::Start,
                        winit::event::TouchPhase::Moved => TouchPhase::Move,
                        winit::event::TouchPhase::Ended => TouchPhase::End,
                        winit::event::TouchPhase::Cancelled => TouchPhase::Cancel,
                    },
                    x: t.location.x as f32 - rx as f32,
                    y: t.location.y as f32 - ry as f32,
                    pen: false,
                    pressure: t.force.map_or(1.0, |f| f.normalized() as f32),
                });
            }
            WindowEvent::Focused(false) => host.send(Input::FocusLost),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let Some(host) = &mut self.host else {
            return;
        };
        host.tick(event_loop);
        // Wake for whichever cart is due next.
        let next = host
            .slots
            .iter()
            .filter_map(|s| s.inst.next_frame())
            .min()
            .unwrap_or(host.start.elapsed() + Duration::from_millis(16));
        event_loop.set_control_flow(ControlFlow::WaitUntil(host.start + next));
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut cart = || args.next().map_or(Cart::None, |p| Cart::Path(p.into()));
    let carts = [cart(), cart()];
    let event_loop = EventLoop::new().expect("an event loop");
    let mut app = App {
        carts: Some(carts),
        host: None,
    };
    event_loop.run_app(&mut app).expect("the event loop");
}

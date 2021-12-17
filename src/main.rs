use std::{mem, rc::Rc, sync::Arc};

use bytemuck::{Pod, Zeroable};
use global::Global;
use lazy_static::lazy_static;
use parking_lot::RwLock;

use switch_board::SwitchBoard;
use wgpu::{util::DeviceExt, BufferUsages};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

mod controls;
mod global;
mod render;
mod sound;
mod switch_board;

pub struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    switch_board: Arc<RwLock<SwitchBoard>>,
    stream: cpal::Stream,
    // NEW!
    render_pipeline: wgpu::RenderPipeline,

    vertex_buf: Rc<wgpu::Buffer>,
    index_buf: Rc<wgpu::Buffer>,
    index_count: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Vertex {
    _pos: [f32; 4],
    _tex_coord: [f32; 2],
}
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalUniforms {
    proj: [[f32; 4]; 4],
    //num_lights: [u32; 4],
}
// struct Entity {
//     mx_world: cgmath::Matrix4<f32>,
//     rotation_speed: f32,
//     color: wgpu::Color,
//     vertex_buf: Rc<wgpu::Buffer>,
//     index_buf: Rc<wgpu::Buffer>,
//     index_format: wgpu::IndexFormat,
//     index_count: usize,
//     uniform_offset: wgpu::DynamicOffset,
// }

fn vertex(pos: [i8; 3], tc: [i8; 2]) -> Vertex {
    Vertex {
        _pos: [pos[0] as f32, pos[1] as f32, pos[2] as f32, 1.0],
        _tex_coord: [tc[0] as f32, tc[1] as f32],
    }
}

fn create_cube() -> (Vec<Vertex>, Vec<u16>) {
    let vertex_data = [
        // top (0, 0, 1)
        vertex([-1, -1, 1], [0, 0]),
        vertex([1, -1, 1], [1, 0]),
        vertex([1, 1, 1], [1, 1]),
        vertex([-1, 1, 1], [0, 1]),
        // bottom (0, 0, -1)
        vertex([-1, 1, -1], [1, 0]),
        vertex([1, 1, -1], [0, 0]),
        vertex([1, -1, -1], [0, 1]),
        vertex([-1, -1, -1], [1, 1]),
        // right (1, 0, 0)
        vertex([1, -1, -1], [0, 0]),
        vertex([1, 1, -1], [1, 0]),
        vertex([1, 1, 1], [1, 1]),
        vertex([1, -1, 1], [0, 1]),
        // left (-1, 0, 0)
        vertex([-1, -1, 1], [1, 0]),
        vertex([-1, 1, 1], [0, 0]),
        vertex([-1, 1, -1], [0, 1]),
        vertex([-1, -1, -1], [1, 1]),
        // front (0, 1, 0)
        vertex([1, 1, -1], [1, 0]),
        vertex([-1, 1, -1], [0, 0]),
        vertex([-1, 1, 1], [0, 1]),
        vertex([1, 1, 1], [1, 1]),
        // back (0, -1, 0)
        vertex([1, -1, 1], [0, 0]),
        vertex([-1, -1, 1], [1, 0]),
        vertex([-1, -1, -1], [1, 1]),
        vertex([1, -1, -1], [0, 1]),
    ];

    let index_data: &[u16] = &[
        0, 1, 2, 2, 3, 0, // top
        4, 5, 6, 6, 7, 4, // bottom
        8, 9, 10, 10, 11, 8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    (vertex_data.to_vec(), index_data.to_vec())
}

pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
);

fn generate_matrix(aspect_ratio: f32) -> cgmath::Matrix4<f32> {
    let mx_projection = cgmath::perspective(cgmath::Deg(45f32), aspect_ratio, 1.0, 10.0);
    let mx_view = cgmath::Matrix4::look_at_rh(
        cgmath::Point3::new(1.5f32, -5.0, 3.0),
        cgmath::Point3::new(0f32, 0.0, 0.0),
        cgmath::Vector3::unit_z(),
    );
    let mx_correction = OPENGL_TO_WGPU_MATRIX;
    mx_correction * mx_projection * mx_view
}

lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref globals: Arc<RwLock<Global>> = Arc::new(RwLock::new(Global::new()));
}

impl State {
    async fn new(window: &Window) -> Self {
        let size = window.inner_size();

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };

        surface.configure(&device, &config);

        let (cube_vertex_data, cube_index_data) = create_cube();
        let cube_vertex_buf = Rc::new(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Cubes Vertex Buffer"),
                contents: bytemuck::cast_slice(&cube_vertex_data),
                usage: wgpu::BufferUsages::VERTEX,
            },
        ));

        let cube_index_buf = Rc::new(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Cubes Index Buffer"),
                contents: bytemuck::cast_slice(&cube_index_data),
                usage: wgpu::BufferUsages::INDEX,
            },
        ));

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
            ],
        });

        // Create other resources
        // let mx_total = generate_matrix(size.width as f32 / size.height as f32);
        // let mx_ref: &[f32; 16] = mx_total.as_ref();
        // let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        //     label: Some("Uniform Buffer"),
        //     contents: bytemuck::cast_slice(mx_ref),
        //     usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        // });

        // let texture_extent = wgpu::Extent3d {
        //     width: 256u32,
        //     height: 256u32,
        //     depth_or_array_layers: 1,
        // };
        // let texture = device.create_texture(&wgpu::TextureDescriptor {
        //     label: None,
        //     size: texture_extent,
        //     mip_level_count: 1,
        //     sample_count: 1,
        //     dimension: wgpu::TextureDimension::D2,
        //     format: wgpu::TextureFormat::R8Uint,
        //     usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
        // });
        // let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        // // Create bind group
        // let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        //     layout: &bind_group_layout,
        //     entries: &[
        //         wgpu::BindGroupEntry {
        //             binding: 0,
        //             resource: uniform_buf.as_entire_binding(),
        //         },
        //         wgpu::BindGroupEntry {
        //             binding: 1,
        //             resource: wgpu::BindingResource::TextureView(&texture_view),
        //         },
        //     ],
        //     label: None,
        // });

        let mx_total = Self::generate_matrix(sc_desc.width as f32 / sc_desc.height as f32);
        let forward_uniforms = GlobalUniforms {
            proj: *mx_total.as_ref(),
            num_lights: [lights.len() as u32, 0, 0, 0],
        };
        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::bytes_of(&forward_uniforms),
            usage: wgpu::BufferUsage::UNIFORM | wgpu::BufferUsage::COPY_DST,
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                // wgpu::BindGroupEntry {
                //     binding: 1,
                //     resource: light_storage_buf.as_entire_binding(),
                // },
                // wgpu::BindGroupEntry {
                //     binding: 2,
                //     resource: wgpu::BindingResource::TextureView(&shadow_view),
                // },
                // wgpu::BindGroupEntry {
                //     binding: 3,
                //     resource: wgpu::BindingResource::Sampler(&shadow_sampler),
                // },
            ],
            label: None,
        });

        let vertex_size = mem::size_of::<Vertex>();
        let (vertex_data, index_data) = create_cube();

        let vertex_buf = Rc::new(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertex_data),
                usage: wgpu::BufferUsages::VERTEX,
            }),
        );

        let index_buf = Rc::new(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(&index_data),
                usage: wgpu::BufferUsages::INDEX,
            }),
        );

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[], //&bind_group_layout
                push_constant_ranges: &[],
            });

        let vertex_buffers = [wgpu::VertexBufferLayout {
            array_stride: vertex_size as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 4 * 4,
                    shader_location: 1,
                },
            ],
        }];

        let vertex_attr = wgpu::vertex_attr_array![0 => Sint8x4, 1 => Sint8x4];
        let vb_desc = wgpu::VertexBufferLayout {
            array_stride: vertex_size as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &vertex_attr,
        };

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                //targets:&[wgpu::],
                buffers: &[vb_desc], //&vertex_buffers, //,
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::REPLACE,
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLAMPING
                clamp_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        });
        //Arc::clone(&self)

        let switch_board = Arc::new(RwLock::new(switch_board::SwitchBoard::new()));
        let dupe_switch = Arc::clone(&switch_board);
        Self {
            surface,
            device,
            queue,
            size,
            config,
            render_pipeline,
            switch_board,
            vertex_buf,
            index_buf,
            index_count: index_data.len(),
            stream: sound::init_sound(dupe_switch).unwrap(),
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
        }
    }

    #[allow(unused_variables)]
    fn input(&mut self, event: &WindowEvent) -> bool {
        false
    }

    fn update(&mut self) {}

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        render::render_loop(self)
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    // State::new uses async code, so we're going to wait for it to finish
    let mut state = pollster::block_on(State::new(&window));

    event_loop.run(move |event, _, control_flow| {
        match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == window.id() => {
                if !state.input(event) {
                    controls::controls_evaluate(event, &mut state, control_flow);
                }
            }
            Event::RedrawRequested(_) => {
                state.update();
                match state.render() {
                    Ok(_) => {}
                    // Reconfigure the surface if lost
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    // The system is out of memory, we should probably quit
                    Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                    // All other errors (Outdated, Timeout) should be resolved by the next frame
                    Err(e) => eprintln!("{:?}", e),
                }
            }
            Event::MainEventsCleared => {
                // RedrawRequested will only trigger once, unless we manually
                // request it.
                window.request_redraw();
            }
            _ => {}
        }
    });
}

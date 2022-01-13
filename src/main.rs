use bytemuck::{Pod, Zeroable};
use lua_define::LuaCore;
use once_cell::sync::OnceCell;
use std::{mem, rc::Rc, sync::Arc};

use cgmath::{Matrix, SquareMatrix};
use ent::Ent;
use global::Global;
use lazy_static::lazy_static;
use parking_lot::{Mutex, RwLock};

use switch_board::SwitchBoard;
use wgpu::{util::DeviceExt, BindGroup, Buffer, BufferUsages};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use crate::ent::EntityUniforms;
use crate::gui::Gui;

mod asset;
mod controls;
mod ent;
mod ent_manager;
mod global;
mod gui;
mod log;
mod lua_define;
mod lua_ent;
mod model;
mod render;
mod sound;
mod switch_board;
mod texture;

const MAX_ENTS: u64 = 100;

pub struct State {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::TextureView,
    size: winit::dpi::PhysicalSize<u32>,
    switch_board: Arc<RwLock<SwitchBoard>>,
    stream: cpal::Stream,
    view_matrix: cgmath::Matrix4<f32>,
    perspective_matrix: cgmath::Matrix4<f32>,
    uniform_buf: Buffer,
    uniform_alignment: u64,
    render_pipeline: wgpu::RenderPipeline,

    entities: Vec<Ent>,
    // vertex_buf: Rc<wgpu::Buffer>,
    // index_buf: Rc<wgpu::Buffer>,
    // index_count: usize,
    entity_bind_group: BindGroup,
    entity_uniform_buf: Buffer,
    bind_group: BindGroup,
    gui: Gui,

    value: f32,
    value2: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalUniforms {
    view: [[f32; 4]; 4],
    persp: [[f32; 4]; 4],
    time: [f32; 4],
}

pub const OPENGL_TO_WGPU_MATRIX: cgmath::Matrix4<f32> = cgmath::Matrix4::new(
    1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
);

fn create_depth_texture(
    config: &wgpu::SurfaceConfiguration,
    device: &wgpu::Device,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width: config.width,
            height: config.height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        label: Some("depth"),
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        // 4.
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual), // 5.
        lod_min_clamp: -100.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    });

    let view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (depth_texture, view, sampler)
}

lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref globals: Arc<RwLock<Global>> = Arc::new(RwLock::new(Global::new()));
    pub static ref lua_master : Arc<Mutex<OnceCell<LuaCore>>> = Arc::new((Mutex::new(OnceCell::new())));
}

impl State {
    async fn new(window: &Window) -> State {
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

        //this order is important, since models can load their own textures we need assets to init first
        crate::texture::init();
        model::init(&device);
        let lua_guard = lua_master.lock();
        let lua_core = lua_guard.get_or_init(|| lua_define::LuaCore::new());
        parking_lot::MutexGuard::unlock_fair(lua_guard);
        crate::asset::init(&device);

        // crate::texture::load_tex("gameboy".to_string());
        // crate::texture::load_tex("guy3".to_string());
        // crate::texture::load_tex("chicken".to_string());
        // crate::texture::load_tex("grass_down".to_string());

        let (diffuse_texture_view, diffuse_sampler, diff_tex) =
            crate::texture::finalize(&device, &queue);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
        };

        surface.configure(&device, &config);

        let entity_uniform_size = mem::size_of::<EntityUniforms>() as wgpu::BufferAddress;

        let uniform_alignment =
            device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
        assert!(entity_uniform_size <= uniform_alignment);
        let mut entities = vec![
            Ent::new(
                cgmath::vec3(0.0, 4.0, 0.0),
                45.,
                1.,
                0.,
                "chicken".to_string(),
                "plane".to_string(),
                0,
                true,
                Some("walker".to_string()),
            ),
            Ent::new(
                cgmath::vec3(4.0, 4.0, 0.0),
                0.,
                1.,
                0.,
                "chicken".to_string(),
                "plane".to_string(),
                uniform_alignment as u32,
                false,
                Some("walker".to_string()),
            ),
            Ent::new(
                cgmath::vec3(6.0, -6.0, 0.0),
                0.,
                1.,
                0.,
                "gameboy".to_string(),
                "plane".to_string(),
                (uniform_alignment * 2) as u32,
                false,
                Some("walker".to_string()),
            ),
            Ent::new(
                cgmath::vec3(-2.0, 4.0, 0.0),
                0.,
                1.,
                0.4,
                "guy3".to_string(),
                "plane".to_string(),
                (uniform_alignment * 3) as u32,
                true,
                Some("walker".to_string()),
            ),
        ];

        let mut offset = 4;
        for i in -3..3 {
            for j in -3..3 {
                entities.push(Ent::new(
                    cgmath::vec3(i as f32 * 8., j as f32 * 8., -0.0001),
                    0.,
                    4.,
                    0.,
                    "grass_down".to_string(),
                    "plane".to_string(),
                    (uniform_alignment * offset) as u32,
                    false,
                    None,
                ));
                offset += 1;
            }
        }
        // let num_entities = 1 + entities.len() as wgpu::BufferAddress;

        let entity_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: MAX_ENTS * uniform_alignment,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_format = wgpu::IndexFormat::Uint16;

        // let test_vec = cgmath::vec3(6.0, 2.0, 2.0);
        // use cgmath::{Decomposed, Deg, InnerSpace, Quaternion, Rotation3};
        // let test_trans = cgmath::Decomposed {
        //     disp: test_vec,
        //     rot: Quaternion::from_axis_angle(test_vec.normalize(), Deg(0.)),
        //     scale: 1.,
        // };

        let local_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(entity_uniform_size),
                    },
                    count: None,
                }],
                label: None,
            });
        let entity_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &local_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &entity_uniform_buf,
                    offset: 0,
                    size: wgpu::BufferSize::new(entity_uniform_size),
                }),
            }],
            label: None,
        });

        let shader = device.create_shader_module(&wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let uniform_size = mem::size_of::<GlobalUniforms>() as wgpu::BufferAddress;
        //println!("struct size is {}", uniform_size);
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(uniform_size), //wgpu::BufferSize::new(64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true }, //wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let (mx_view, mx_persp) =
            render::generate_matrix(size.width as f32 / size.height as f32, 0.);

        let render_uniforms = GlobalUniforms {
            view: *mx_view.as_ref(),
            persp: *mx_persp.as_ref(),
            time: [0f32, 0f32, 0f32, 0f32],
            //num_lights: [lights.len() as u32, 0, 0, 0],
        };

        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::bytes_of(&render_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // Create bind group
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&diffuse_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&diffuse_sampler),
                },
            ],
            label: None,
        });

        let vertex_size = mem::size_of::<crate::model::Vertex>();

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout, &local_bind_group_layout], //&bind_group_layout
                push_constant_ranges: &[],
            });

        // let vertex_buffers = [wgpu::VertexBufferLayout {
        //     array_stride: vertex_size as wgpu::BufferAddress,
        //     step_mode: wgpu::VertexStepMode::Vertex,
        //     attributes: &[
        //         wgpu::VertexAttribute {
        //             format: wgpu::VertexFormat::Float32x4,
        //             offset: 0,
        //             shader_location: 0,
        //         },
        //         wgpu::VertexAttribute {
        //             format: wgpu::VertexFormat::Float32x2,
        //             offset: 4 * 4,
        //             shader_location: 1,
        //         },
        //     ],
        // }];

        let vertex_attr = wgpu::vertex_attr_array![0 => Sint16x4, 1 => Sint8x4, 2=> Float32x2];
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
                    // blend: Some(wgpu::BlendState {
                    //     color: wgpu::BlendComponent::OVER,
                    //     alpha: wgpu::BlendComponent::OVER,
                    // }),
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    // blend: Some(wgpu::Blend {
                    //     src_factor: wgpu::BlendFactor::One,
                    //     dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    //     operation: wgpu::BlendOperation::Add,
                    // }),
                    // write_mask: wgpu::ColorWrites::ALL,
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
                //clamp_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less, // 1.
                stencil: wgpu::StencilState::default(),     // 2.
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });
        //Arc::clone(&self)

        let depth = create_depth_texture(&config, &device);

        let switch_board = Arc::new(RwLock::new(switch_board::SwitchBoard::new()));
        let dupe_switch = Arc::clone(&switch_board);

        //Gui

        // let gui_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        //     label: None,
        //     entries: &[
        //         wgpu::BindGroupLayoutEntry {
        //             binding: 0,
        //             visibility: wgpu::ShaderStages::VERTEX,
        //             ty: wgpu::BindingType::Buffer {
        //                 ty: wgpu::BufferBindingType::Uniform,
        //                 has_dynamic_offset: false,
        //                 min_binding_size: wgpu::BufferSize::new(uniform_size), //wgpu::BufferSize::new(64),
        //             },
        //             count: None,
        //         },
        //         wgpu::BindGroupLayoutEntry {
        //             binding: 1,
        //             visibility: wgpu::ShaderStages::FRAGMENT,
        //             ty: wgpu::BindingType::Texture {
        //                 multisampled: false,
        //                 sample_type: wgpu::TextureSampleType::Float { filterable: true }, //wgpu::TextureSampleType::Uint,
        //                 view_dimension: wgpu::TextureViewDimension::D2,
        //             },
        //             count: None,
        //         },
        //         wgpu::BindGroupLayoutEntry {
        //             binding: 2,
        //             visibility: wgpu::ShaderStages::FRAGMENT,
        //             ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        //             count: None,
        //         },
        //     ],
        // });

        let (gui_texture_view, gui_sampler, gui_texture, gui_image) =
            gui::init_image(&device, &queue, size.width as f32 / size.height as f32);
        let gui_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&gui_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&gui_sampler),
                },
            ],
            label: None,
        });

        let gui_vertex_attr = wgpu::vertex_attr_array![0 => Sint16x4, 1 => Sint8x4, 2=> Float32x2];
        let gui_desc = wgpu::VertexBufferLayout {
            array_stride: vertex_size as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &gui_vertex_attr,
        };

        let gui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Gui Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "gui_vs_main",
                //targets:&[wgpu::],
                buffers: &[], //&vertex_buffers, //,
            },

            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "gui_fs_main",
                targets: &[wgpu::ColorTargetState {
                    format: config.format,
                    // blend: Some(wgpu::BlendState {
                    //     color: wgpu::BlendComponent::OVER,
                    //     alpha: wgpu::BlendComponent::OVER,
                    // }),
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    // blend: Some(wgpu::Blend {
                    //     src_factor: wgpu::BlendFactor::One,
                    //     dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    //     operation: wgpu::BlendOperation::Add,
                    // }),
                    // write_mask: wgpu::ColorWrites::ALL,
                    write_mask: wgpu::ColorWrites::ALL,
                }],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                polygon_mode: wgpu::PolygonMode::Fill,
                // Requires Features::DEPTH_CLAMPING
                //clamp_depth: false,
                // Requires Features::CONSERVATIVE_RASTERIZATION
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less, // 1.
                stencil: wgpu::StencilState::default(),     // 2.
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        Self {
            surface,
            device,
            queue,
            size,
            config,
            depth_texture: depth.1,
            uniform_buf,
            uniform_alignment,
            view_matrix: mx_view,
            perspective_matrix: mx_persp,
            render_pipeline,
            switch_board,
            entities,
            gui: Gui::new(gui_pipeline, gui_group, gui_texture, gui_image),
            bind_group,
            entity_bind_group,
            entity_uniform_buf,
            stream: sound::init_sound(dupe_switch).unwrap(),
            value: 0.,
            value2: 0.,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            let d = create_depth_texture(&self.config, &self.device);
            self.depth_texture = d.1;
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
    window.set_title("Petrichor");
    // window.set_out(winit::dpi::LogicalSize::new(256, 256));
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

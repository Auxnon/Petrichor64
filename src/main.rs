#![windows_subsystem = "windows"]

use bundle::BundleManager;
use bytemuck::{Pod, Zeroable};
use command::MainCommmand;
use ent_manager::EntManager;
use global::Global;
use itertools::Itertools;
use lua_define::{LuaCore, MainPacket};
use model::ModelManager;
use sound::SoundPacket;
use std::{
    mem,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
};
use texture::TexManager;
// use tracy::frame;
use crate::gui::Gui;
use crate::{ent::EntityUniforms, post::Post};
use glam::{vec2, vec3, Mat4};
use parking_lot::RwLock;
use switch_board::SwitchBoard;
use wgpu::{util::DeviceExt, BindGroup, Buffer, CompositeAlphaMode, Texture};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    // platform::macos::WindowExtMacOS,
    window::{Window, WindowBuilder},
};
use world::World;

mod asset;
mod bundle;
mod command;
mod controls;
mod ent;
mod ent_manager;
mod global;
mod gui;
mod log;
mod lua_define;
mod lua_ent;
mod model;
#[cfg(feature = "online_capable")]
mod online;
mod pad;
mod parse;
mod post;
mod ray;
mod render;
mod sound;
mod switch_board;
mod template;
mod texture;
mod tile;
mod world;
mod zip_pal;

const MAX_ENTS: u64 = 10000;
/** All centralized engines and factories to be passed around in the main thread */
pub struct Core {
    surface: wgpu::Surface,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::TextureView,
    size: winit::dpi::PhysicalSize<u32>,
    switch_board: Arc<RwLock<SwitchBoard>>,
    global: Global,
    /** despite it's unuse, this stream needs to persist or sound will not occur */
    _stream: Option<cpal::Stream>,
    singer: Sender<SoundPacket>,
    uniform_buf: Buffer,
    uniform_alignment: u64,
    render_pipeline: wgpu::RenderPipeline,
    world: World,
    pitcher: Sender<MainPacket>,
    catcher: Receiver<MainPacket>,
    entity_bind_group: BindGroup,
    entity_uniform_buf: Buffer,
    main_bind_group: BindGroup,
    master_texture: Texture,
    gui: Gui,
    post: Post,
    loop_helper: spin_sleep::LoopHelper,
    // lua_master: LuaCore,
    tex_manager: TexManager,
    model_manager: ModelManager,
    ent_manager: EntManager,
    input_manager: winit_input_helper::WinitInputHelper,
    bundle_manager: BundleManager,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalUniforms {
    view: [[f32; 4]; 4],
    persp: [[f32; 4]; 4],
    adjustments: [[f32; 4]; 4],
}

// pub const OPENGL_TO_WGPU_MATRIX: Mat4 = Mat4:new()
//     1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
// };

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

//DEV consider atomics such as AtomicU8 for switch_board or lazy static primatives

impl Core {
    async fn new(window: &Window) -> Core {
        // crate::texture::save_audio_buffer(&vec![255u8; 1024]);
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
                    limits: wgpu::Limits {
                        max_storage_textures_per_shader_stage: 8,
                        ..wgpu::Limits::default()
                    },
                },
                None, // Trace path
            )
            .await
            .unwrap();

        //this order is important, since models can load their own textures we need assets to init first
        let tex_manager = crate::texture::TexManager::new();
        let model_manager = ModelManager::init(&device);
        let mut ent_manager = EntManager::new(&device);

        // crate::texture::load_tex("gameboy".to_string());
        // crate::texture::load_tex("guy3".to_string());
        // crate::texture::load_tex("chicken".to_string());
        // crate::texture::load_tex("grass_down".to_string());

        let (diffuse_texture_view, diffuse_sampler, diff_tex) =
            tex_manager.finalize(&device, &queue);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb, //Bgra8UnormSrgb
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Opaque,
        };

        surface.configure(&device, &config);

        let entity_uniform_size = mem::size_of::<EntityUniforms>() as wgpu::BufferAddress;

        let uniform_alignment =
            device.limits().min_uniform_buffer_offset_alignment as wgpu::BufferAddress;
        assert!(entity_uniform_size <= uniform_alignment);

        let entity_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: MAX_ENTS * uniform_alignment,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // let index_format = wgpu::IndexFormat::Uint16;

        let entity_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(entity_uniform_size),
                    },
                    count: None,
                }],
                label: Some("entity bind group layout"),
            });
        let entity_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &entity_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &entity_uniform_buf,
                    offset: 0,
                    size: wgpu::BufferSize::new(entity_uniform_size),
                }),
            }],
            label: Some("entity bind group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/shader.wgsl").into()),
        });

        let uniform_size = mem::size_of::<GlobalUniforms>() as wgpu::BufferAddress;

        let main_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("main bind group layout"),
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

        // let gui_bind_group_layout =
        //     device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        //         label: None,
        //         entries: &[
        //             wgpu::BindGroupLayoutEntry {
        //                 binding: 0,
        //                 visibility: wgpu::ShaderStages::VERTEX,
        //                 ty: wgpu::BindingType::Buffer {
        //                     ty: wgpu::BufferBindingType::Uniform,
        //                     has_dynamic_offset: false,
        //                     min_binding_size: wgpu::BufferSize::new(uniform_size), //wgpu::BufferSize::new(64),
        //                 },
        //                 count: None,
        //             },
        //             wgpu::BindGroupLayoutEntry {
        //                 binding: 1,
        //                 visibility: wgpu::ShaderStages::FRAGMENT,
        //                 ty: wgpu::BindingType::Texture {
        //                     multisampled: false,
        //                     sample_type: wgpu::TextureSampleType::Float { filterable: true }, //wgpu::TextureSampleType::Uint,
        //                     view_dimension: wgpu::TextureViewDimension::D2,
        //                 },
        //                 count: None,
        //             },
        //             wgpu::BindGroupLayoutEntry {
        //                 binding: 2,
        //                 visibility: wgpu::ShaderStages::FRAGMENT,
        //                 ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        //                 count: None,
        //             },
        //         ],
        //     });

        let (mx_view, mx_persp, _mx_model) = render::generate_matrix(
            size.width as f32 / size.height as f32,
            0.,
            vec3(0., 0., 0.),
            vec2(0., 0.),
        );

        let render_uniforms = GlobalUniforms {
            view: mx_view.to_cols_array_2d(),
            persp: mx_persp.to_cols_array_2d(),
            adjustments: Mat4::ZERO.to_cols_array_2d(),
            //num_lights: [lights.len() as u32, 0, 0, 0],
        };

        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::bytes_of(&render_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let flat_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sky Render Pipeline Layout"),
            bind_group_layouts: &[&main_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    // &main_bind_group_layout,
                    &main_bind_group_layout,
                    &entity_bind_group_layout,
                    // &main_bind_group_layout,
                ], //&bind_group_layout
                push_constant_ranges: &[],
            });

        // let vertex_attr = wgpu::vertex_attr_array![0 => Sint16x4, 1 => Sint8x4, 2=> Float32x2];
        // let vb_desc = wgpu::VertexBufferLayout {
        //     array_stride: vertex_size as wgpu::BufferAddress,
        //     step_mode: wgpu::VertexStepMode::Vertex,
        //     attributes: &vertex_attr,
        // };

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                //targets:&[wgpu::],
                buffers: &[
                    crate::model::Vertex::desc(),
                    crate::ent::EntityUniforms::desc(),
                ], //&vertex_buffers, //,
            },

            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
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
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Some(wgpu::Face::Back), //DEV front face cull mode should be lua controlled?
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

        let depth = create_depth_texture(&config, &device);

        let switch_board = Arc::new(RwLock::new(switch_board::SwitchBoard::new()));

        //Gui
        let (gui_texture_view, gui_sampler, gui_texture, gui_image) =
            gui::init_image(&device, &queue, size.width as f32 / size.height as f32);

        let (sky_texture_view, sky_sampler, sky_texture, sky_image) =
            gui::init_image(&device, &queue, size.width as f32 / size.height as f32);

        let sky_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &main_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&sky_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sky_sampler),
                },
            ],
            label: None,
        });

        let gui_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &main_bind_group_layout,
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

        // Create main bind group
        let main_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &main_bind_group_layout,
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

        let gui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Gui Pipeline"),
            layout: Some(&flat_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "gui_vs_main",
                buffers: &[], //&vertex_buffers, //,
            },

            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "gui_fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
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

        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Sky Pipeline"),
            layout: Some(&flat_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "sky_vs_main",
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "sky_fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let mut gui = Gui::new(
            gui_pipeline,
            sky_pipeline,
            gui_group,
            gui_texture,
            sky_group,
            sky_texture,
            gui_image,
        );
        gui.add_text("initialized".to_string());
        // gui.add_img(&"map.tile.png".to_string());

        let global = Global::new();
        if global.console {
            gui.enable_console()
        }

        let post = Post::new(&config, &device, &shader, &uniform_buf, uniform_size);

        let world = World::new();

        let loop_helper = spin_sleep::LoopHelper::builder()
            .report_interval_s(0.5) // report every half a second
            .build_with_target_rate(60.0); // limit to X FPS if possible

        let (stream, singer) = sound::init();
        let stream_result = match stream {
            Ok(stream) => Some(stream),
            Err(e) => {
                crate::log::log(format!("sound stream error, continuing in silence!: {}", e));
                None
            }
        };
        ent_manager.uniform_alignment = uniform_alignment as u32;
        let input_manager = winit_input_helper::WinitInputHelper::new();
        let (pitcher, catcher) = channel::<MainPacket>();
        Self {
            surface,
            device,
            queue,
            size,
            config,
            depth_texture: depth.1,
            uniform_buf,
            uniform_alignment,
            // view_matrix: mx_view,
            // perspective_matrix: mx_persp,
            render_pipeline,
            global,
            switch_board: Arc::clone(&switch_board),
            post,
            gui,
            main_bind_group,
            entity_bind_group,
            entity_uniform_buf,
            _stream: stream_result,
            singer,
            world,
            master_texture: diff_tex,
            loop_helper,
            //
            tex_manager,
            model_manager,
            ent_manager,
            pitcher,
            catcher,
            input_manager,
            bundle_manager: BundleManager::new(),
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            // println!("1resize {} {}", self.size.width, self.size.height);
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            let d = create_depth_texture(&self.config, &self.device);
            self.depth_texture = d.1;
            self.post.resize(&self.device, new_size, &self.uniform_buf);
        }
    }

    // #[allow(unused_variables)]
    // fn input(&mut self, event: &WindowEvent) -> bool {
    //     false
    // }

    fn update(&mut self) {
        let mut mutations = vec![];

        for (id, p) in self.catcher.try_iter() {
            match p {
                MainCommmand::CamPos(v) => {
                    self.global.camera_pos = v;
                    // println!("🧲 eyup pos{} {} {}", p.1, p.2, p.3);
                }
                MainCommmand::CamRot(v) => {
                    self.global.simple_cam_rot = v;
                    // println!("🧲 eyup rot{} {} {}", p.1, p.2, p.3);
                }
                MainCommmand::Sky() => {
                    self.gui.target_sky();
                }
                MainCommmand::Gui() => {
                    self.gui.target_gui();
                }
                MainCommmand::Fill(v) => {
                    self.gui.fill(v.x, v.y, v.z, v.w);
                }
                MainCommmand::Square(x, y, w, h) => {
                    self.gui.square(x, y, w, h);
                }
                MainCommmand::Line(x, y, x2, y2) => {
                    self.gui.line(x, y, x2, y2);
                }
                MainCommmand::Text(s, x, y) => self.gui.direct_text(&s, false, x, y),
                MainCommmand::DrawImg(s, x, y) => {
                    self.gui.draw_image(&mut self.tex_manager, &s, false, x, y)
                }
                MainCommmand::GetImg(s, tx) => {
                    tx.send(self.tex_manager.get_img(&s));
                }
                MainCommmand::Pixel(x, y, v) => self.gui.pixel(x, y, v.x, v.y, v.z, v.w),
                MainCommmand::Anim(name, items, speed) => {
                    let frames = items
                        .iter()
                        .map(|i| self.tex_manager.get_tex(i))
                        .collect_vec();
                    self.tex_manager.animations.insert(
                        name,
                        crate::texture::Anim {
                            frames,
                            speed,
                            once: false,
                        },
                    );
                }
                MainCommmand::Clear() => self.gui.clean(),
                MainCommmand::Make(m, tx) => {
                    // self.gui.draw_image(&s, false, x as i64, y as i64)
                    // println!("this far");
                    if m.len() == 7 {
                        let m2 = vec![
                            m[1].clone(),
                            m[2].clone(),
                            m[3].clone(),
                            m[4].clone(),
                            m[5].clone(),
                            m[6].clone(),
                        ];
                        self.model_manager.edit_cube(
                            &mut self.world,
                            &self.tex_manager,
                            m[0].clone(),
                            m2,
                            &self.device,
                        );
                        // println!("this far2");

                        tx.send(0);
                        // println!("this far3");
                    }
                }
                MainCommmand::Spawn(lent) => {
                    //asset, x, y, z, s, count, tx) => {
                    // let mut v = vec![];
                    // for i in 0..count {
                    //     let index = self.ent_manager.id_counter;
                    //     self.ent_manager.id_counter += 1;
                    //     let ent =
                    //         crate::lua_ent::LuaEnt::new(index, asset.clone(), x, y, z, s);
                    //     // // Rc<RefCell
                    //     let wrapped = Arc::new(std::sync::Mutex::new(ent));
                    //     v.push(Arc::clone(&wrapped));
                    //     self.ent_manager.create_from_lua(
                    //         &self.tex_manager,
                    //         &self.model_manager,
                    //         wrapped,
                    //     );
                    // }
                    self.ent_manager
                        .create_from_lua(&self.tex_manager, &self.model_manager, lent);
                    // tx.send(v);
                }
                MainCommmand::Group(parent, child, tx) => {
                    self.ent_manager.group(parent, child);
                    tx.send(true);
                }
                MainCommmand::Kill(id) => self.ent_manager.kill_ent(id),
                MainCommmand::Globals(table) => {
                    println!("global remap");
                    for (k, v) in table.iter() {
                        println!("global map {} {}", k, v);
                        match k.as_str() {
                            "resolution" => self.global.screen_effects.crt_resolution = *v,
                            "curvature" => self.global.screen_effects.corner_harshness = *v,
                            "flatness" => self.global.screen_effects.corner_ease = *v,
                            "dark" => self.global.screen_effects.dark_factor = *v,
                            "bleed" => self.global.screen_effects.lumen_threshold = *v,
                            "glitch" => self.global.screen_effects.glitchiness = *v,
                            "high" => self.global.screen_effects.high_range = *v,
                            "low" => self.global.screen_effects.low_range = *v,
                            "modernize" => self.global.screen_effects.modernize = *v,
                            _ => {}
                        }
                    }
                }
                MainCommmand::AsyncError(e) => {
                    let ee = e
                        .split_inclusive(|c| c == '[' || c == ']')
                        .enumerate()
                        .filter(|(i, s)| i % 2 == 0)
                        .map(|x| x.1)
                        .join("...]");
                    // .collect::<Vec<String>>();
                    // s = re.sub(r'\(.*?\)', '', s)
                    let s = format!("async error: {}", ee);
                    println!("{}", s);
                    crate::log::log(s);
                }
                MainCommmand::BundleDropped(b) => self.bundle_manager.reclaim_resources(b),
                MainCommmand::Subload(file, is_overlay) => {
                    mutations.push((id, MainCommmand::Subload(file, is_overlay)));
                }
                MainCommmand::Reload() => {
                    // println!("resetttt");
                    mutations.push((id, MainCommmand::Reload()));
                }
                MainCommmand::AsyncGui(g, b) => {
                    self.gui.replace_image(g, b);
                }
                _ => {}
            };
        }

        if !mutations.is_empty() {
            for (id, m) in mutations {
                match m {
                    MainCommmand::Reload() => crate::command::reload(self, id),
                    MainCommmand::Subload(file, is_overlay) => {
                        crate::command::load(self, Some(file), None, None, Some((id, is_overlay)));
                    }
                    _ => {}
                }
            }
        }
        self.ent_manager.check_ents(
            self.global.iteration,
            &self.tex_manager,
            &self.model_manager,
        );

        self.global.iteration += 1;
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let _delta = self.loop_helper.loop_start();

        // or .loop_start_s() for f64 seconds
        if let Some(fps) = self.loop_helper.report_rate() {
            //  let current_fps = Some(fps);
            self.global.fps = fps;
        }
        self.global.delayed += 1;
        if self.global.delayed >= 128 {
            self.global.delayed = 0;
            println!("fps::{}", self.global.fps);
        }

        let s = render::render_loop(self, self.global.iteration);
        self.loop_helper.loop_sleep(); //DEV better way to sleep that allows maincommands to come through but pauses render?

        // match self.loop_helper.report_rate() {
        //     Some(t) => println!("now we over {}", t),
        //     None => println!("not yet"),
        // };
        s
    }
}

fn main() {
    crate::parse::test(&"test.lua".to_string());
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize::new(640i32, 480i32))
        .build(&event_loop)
        .unwrap();
    window.set_title("Petrichor");

    // State::new uses async code, so we're going to wait for it to finish

    let mut core = pollster::block_on(Core::new(&window));

    // unsafe {
    //     tracy::startup_tracy();
    // }
    let mut bits = ([false; 256], [0.; 4]);

    match crate::asset::check_for_auto() {
        Some(s) => {
            core.global.console = false;
            core.gui.disable_console();
            crate::command::hard_reset(&mut core);
            crate::command::load_from_string(&mut core, Some(s));
        }
        _ => {
            #[cfg(feature = "include_auto")]
            {
                core.global.console = false;
                core.gui.disable_console();
                crate::command::reload(&mut core);
            }

            #[cfg(not(feature = "include_auto"))]
            {
                crate::command::load_empty(&mut core);
            }
        }
    }

    // :reload(core);

    event_loop.run(move |event, _, control_flow| {
        controls::bit_check(&event, &mut bits);
        bits.1[0] = core.global.mouse_pos.x;
        bits.1[1] = core.global.mouse_pos.y;
        // println!("bits {:?}", bits.0);

        if core.input_manager.update(&event) {
            controls::controls_evaluate(&mut core, control_flow);
            // frame!("START");
            core.update();
            core.bundle_manager.call_loop(bits);

            match core.render() {
                Ok(_) => {}
                // Reconfigure the surface if lost
                Err(wgpu::SurfaceError::Lost) => core.resize(core.size),
                // The system is out of memory, we should probably quit
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                // All other errors (Outdated, Timeout) should be resolved by the next frame
                Err(e) => eprintln!("{:?}", e),
            }
            // frame!("END");
            // frame!();
        }
        match event {
            Event::WindowEvent {
                ref event,
                window_id: _,
            } => match event {
                WindowEvent::Resized(physical_size) => {
                    core.resize(*physical_size);
                }
                WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                    // new_inner_size is &&mut so we have to dereference it twice
                    core.resize(**new_inner_size);
                }
                _ => {}
            },
            _ => {}
        }
    });

    // unsafe {
    //     tracy::shutdown_tracy();
    // }
}

// fn log(str: String) {
//     crate::log::log(format!("main::{}", str));
// }

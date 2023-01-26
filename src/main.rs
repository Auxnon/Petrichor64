#![windows_subsystem = "windows"]
use bundle::BundleManager;
use bytemuck::{Pod, Zeroable};
use command::MainCommmand;
use controls::ControlState;
use ent_manager::{EntManager, InstanceBuffer};
use global::{Global, GuiParams, StateChange};
use itertools::Itertools;
use lua_define::{LuaCore, MainPacket};
use model::ModelManager;
use rustc_hash::FxHashMap;
#[cfg(feature = "audio")]
use sound::{SoundCommand, SoundPacket};
use std::{
    mem,
    rc::Rc,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc,
    },
};
use texture::TexManager;
use types::ValueMap;
// use tracy::frame;
use crate::{ent::EntityUniforms, global::GuiStyle, post::Post, texture::TexTuple};
use crate::{gui::Gui, log::LogType};
use glam::{vec2, vec3, Mat4};
use parking_lot::RwLock;
use switch_board::SwitchBoard;
use wgpu::{util::DeviceExt, BindGroup, Buffer, CompositeAlphaMode, Texture};
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::*,
    event_loop::{ControlFlow, EventLoop},
    // platform::macos::WindowExtMacOS,
    window::{CursorGrabMode, Window, WindowBuilder},
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
mod lua_img;
mod model;
#[cfg(feature = "online_capable")]
mod online;
mod pad;
mod parse;
mod post;
mod ray;
mod render;
#[cfg(feature = "audio")]
mod sound;
mod switch_board;
mod template;
mod texture;
mod tile;
mod types;
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
    #[cfg(feature = "audio")]
    _stream: Option<cpal::Stream>,
    #[cfg(feature = "audio")]
    singer: Sender<SoundCommand>,
    uniform_buf: Buffer,
    uniform_alignment: u64,
    render_pipeline: wgpu::RenderPipeline,
    world: World,
    pitcher: Sender<MainPacket>,
    catcher: Receiver<MainPacket>,
    entity_bind_group: BindGroup,
    entity_uniform_buf: Buffer,
    main_bind_group: BindGroup,
    main_bind_group_layout: wgpu::BindGroupLayout,
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
    win_ref: Rc<Window>,
    loggy: log::Loggy,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalUniforms {
    view: [[f32; 4]; 4],
    persp: [[f32; 4]; 4],
    adjustments: [[f32; 4]; 4],
    specs: [f32; 4],
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
    async fn new(rwindow: Rc<Window>) -> Core {
        // crate::texture::save_audio_buffer(&vec![255u8; 1024]);
        let window = &*rwindow;
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
                None,
            )
            .await
            .unwrap();

        //this order is important, since models can load their own textures we need assets to init first
        let tex_manager = crate::texture::TexManager::new();
        let model_manager = ModelManager::init(&device);
        let mut ent_manager = EntManager::new(&device);

        let TexTuple {
            view: diffuse_texture_view,
            sampler: diffuse_sampler,
            texture: diff_tex,
        } = tex_manager.finalize(&device, &queue);

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

        let (mx_view, mx_persp, _mx_model) = render::generate_matrix(
            size.width as f32 / size.height as f32,
            vec3(0., 0., 0.),
            vec2(0., 0.),
        );

        let render_uniforms = GlobalUniforms {
            view: mx_view.to_cols_array_2d(),
            persp: mx_persp.to_cols_array_2d(),
            adjustments: Mat4::ZERO.to_cols_array_2d(),
            specs: [0.0, 0.0, 0.0, 0.0],
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

        let global = Global::new();

        //Gui

        let psize = winit::dpi::PhysicalSize::new(1280, 960);
        let gui_scaled = Self::compute_gui_size(&global.gui_params, psize);
        println!("gui_scaled: {:?}", gui_scaled);
        // let (gui_bundle, gui_image) = gui::init_image(&device, &queue, gui_scaled);

        // let (sky_bundle, sky_image) = gui::init_image(&device, &queue, gui_scaled);

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

        let mut loggy = log::Loggy::new();

        let mut gui = Gui::new(
            gui_pipeline,
            sky_pipeline,
            &main_bind_group_layout,
            &uniform_buf,
            &device,
            &queue,
            gui_scaled,
            &mut loggy,
        );
        let (w, h) = gui.get_console_size();
        loggy.set_dimensions(w, h);
        gui.add_text("initialized".to_string());

        if global.console {
            gui.enable_console(&loggy)
        }

        let post = Post::new(&config, &device, &shader, &uniform_buf, uniform_size);

        let world = World::new(loggy.make_sender());

        let loop_helper = spin_sleep::LoopHelper::builder()
            .report_interval_s(0.5) // report every half a second
            .build_with_target_rate(120.0); // limit to X FPS if possible

        #[cfg(feature = "audio")]
        let (stream, singer) = sound::init();
        #[cfg(feature = "audio")]
        let stream_result = match stream {
            Ok(stream) => Some(stream),
            Err(e) => {
                loggy.log(
                    LogType::CoreError,
                    &format!("sound stream error, continuing in silence!: {}", e),
                );
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
            main_bind_group_layout,
            entity_bind_group,
            entity_uniform_buf,
            #[cfg(feature = "audio")]
            _stream: stream_result,
            #[cfg(feature = "audio")]
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
            win_ref: rwindow,
            loggy,
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            // self.config.
            // println!("physical resize {} {}", self.size.width, self.size.height);
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            if self.global.state_delay == 0 {
                self.global.state_changes.push(StateChange::Resized);
            }
            self.global.state_delay = 15;
            self.global.is_state_changed = true;
        }
    }

    pub fn debounced_resize(&mut self) {
        let new_size = self.size;
        self.surface.configure(&self.device, &self.config);
        let d = create_depth_texture(&self.config, &self.device);
        self.depth_texture = d.1;
        self.post.resize(&self.device, new_size, &self.uniform_buf);
        let gui_scaled = Self::compute_gui_size(&self.global.gui_params, new_size);
        // println!(
        //     "gui resize {} {} new size {} {}",
        //     gui_scaled.0, gui_scaled.1, new_size.width, new_size.height
        // );
        self.gui.resize(
            gui_scaled,
            &self.device,
            &self.queue,
            &self.main_bind_group_layout,
            &self.uniform_buf,
        );
        let (con_w, con_h) = self.gui.get_console_size();
        self.loggy.set_dimensions(con_w, con_h);
        self.bundle_manager.resize(gui_scaled.0, gui_scaled.1);
    }

    fn compute_gui_size(gui_params: &GuiParams, new_size: PhysicalSize<u32>) -> (u32, u32) {
        let gui_size = gui_params.resolution;
        match gui_params.style {
            GuiStyle::Aspect => {
                let aspect = new_size.width as f32 / new_size.height as f32;
                let gaspect = gui_size.0 as f32 / gui_size.1 as f32;
                // preserve aspect ratio
                if aspect > gaspect {
                    //wider
                    let new_width = (gui_size.1 as f32 * aspect) as u32;
                    (new_width, gui_size.1)
                } else {
                    //taller
                    let new_height = (gui_size.0 as f32 / aspect) as u32;
                    (gui_size.0, new_height)
                }
            }
            GuiStyle::Width => {
                let aspect = new_size.width as f32 / new_size.height as f32;
                let new_height = (gui_size.0 as f32 / aspect) as u32;
                (gui_size.0, new_height)
            }
            GuiStyle::Height => {
                let aspect = new_size.width as f32 / new_size.height as f32;
                let new_width = (gui_size.1 as f32 * aspect) as u32;
                (new_width, gui_size.1)
            }
        }
    }

    fn update(&mut self, completed_bundles: &mut FxHashMap<u8, bool>) -> Option<InstanceBuffer> {
        let mut mutations = vec![];
        let mut loop_complete = false;
        for (id, p) in self.catcher.try_iter() {
            match p {
                MainCommmand::Cam(p, r) => {
                    if let Some(pos) = p {
                        self.global.cam_pos = pos;
                    }
                    if let Some(rot) = r {
                        self.global.simple_cam_rot = rot;
                    }
                }
                MainCommmand::DrawImg(s, x, y) => {
                    self.gui.draw_image(&mut self.tex_manager, &s, false, x, y)
                }
                MainCommmand::GetImg(s, tx) => {
                    tx.send(self.tex_manager.get_img(&s));
                }
                MainCommmand::SetImg(s, im) => {
                    self.tex_manager.overwrite_texture(
                        &s,
                        im,
                        &mut self.world,
                        id,
                        &mut self.loggy,
                    );
                    self.tex_manager
                        .refinalize(&self.queue, &self.master_texture);
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
                MainCommmand::Model(name, texture, v, i, u, style, tx) => {
                    self.model_manager.upsert_model(
                        &self.device,
                        &self.tex_manager,
                        &mut self.world,
                        id,
                        &name,
                        texture,
                        v,
                        i,
                        u,
                        style,
                        &mut self.loggy,
                        self.global.debug,
                    );

                    tx.send(0);
                    // name, v, i, u);
                }
                MainCommmand::ListModel(s, bundles, tx) => {
                    let list = self.model_manager.search_model(&s, bundles);
                    tx.send(list);
                }
                MainCommmand::Make(m, tx) => {
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
                            id,
                            &self.tex_manager,
                            m[0].clone(),
                            m2,
                            &self.device,
                        );

                        tx.send(0);
                    }
                }
                MainCommmand::Spawn(lent) => {
                    self.ent_manager
                        .create_from_lua(&self.tex_manager, &self.model_manager, lent);
                }
                MainCommmand::Group(parent, child, tx) => {
                    self.ent_manager.group(parent, child);
                    tx.send(true);
                }
                MainCommmand::Kill(id) => self.ent_manager.kill_ent(id),
                MainCommmand::Globals(table) => {
                    for (k, v) in table.iter() {
                        match k.as_str() {
                            "resolution" => {
                                self.global.screen_effects.crt_resolution = Self::val2float(v)
                            }
                            "curvature" => {
                                self.global.screen_effects.corner_harshness = Self::val2float(v)
                            }
                            "flatness" => {
                                self.global.screen_effects.corner_ease = Self::val2float(v)
                            }
                            "dark" => self.global.screen_effects.dark_factor = Self::val2float(v),
                            "bleed" => {
                                self.global.screen_effects.lumen_threshold = Self::val2float(v)
                            }
                            "glitch" => self.global.screen_effects.glitchiness = Self::val2vec3(v),
                            "high" => self.global.screen_effects.high_range = Self::val2float(v),
                            "low" => self.global.screen_effects.low_range = Self::val2float(v),
                            "modernize" => {
                                self.global.screen_effects.modernize = Self::val2float(v)
                            }
                            "fog" => self.global.screen_effects.fog = Self::val2float(v),
                            "fullscreen" => {
                                self.global.fullscreen = Self::val2bool(v);
                                self.check_fullscreen();
                                self.global.fullscreen_state = self.global.fullscreen;
                            }
                            "mouse_grab" => self.global.mouse_grab = Self::val2bool(v),
                            "size" => {
                                let arr = Self::val2array(v);
                                if arr.len() > 0 {
                                    self.win_ref.set_inner_size(LogicalSize::new(
                                        arr[0].clamp(10., f32::INFINITY) as u32,
                                        self.size.height,
                                    ));
                                    if arr.len() > 1 {
                                        self.win_ref.set_inner_size(LogicalSize::new(
                                            self.size.width,
                                            arr[1].clamp(10., f32::INFINITY) as u32,
                                        ));
                                    }
                                }
                            }
                            "title" => {
                                if let Some(s) = Self::val2string(v) {
                                    self.win_ref.set_title(&s);
                                }
                            }
                            "lock" => {
                                self.global.console = false;
                                self.gui.disable_console();
                                self.global.locked = true;
                            }

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
                    let s = format!("async error: {}", ee);
                    // println!("{}", s);
                    self.loggy.log(LogType::LuaError, &s);
                    // crate::log::log(s);
                }
                MainCommmand::BundleDropped(b) => {
                    completed_bundles.remove(&id);
                    self.bundle_manager.reclaim_resources(b)
                }
                MainCommmand::Subload(file, is_overlay) => {
                    mutations.push((id, MainCommmand::Subload(file, is_overlay)));
                }
                MainCommmand::Reload() => {
                    mutations.push((id, MainCommmand::Reload()));
                }

                MainCommmand::WorldSync(chunks, dropped) => {
                    self.world
                        .process_sync(id, chunks, dropped, &self.model_manager, &self.device);
                }
                MainCommmand::Stats() => {
                    self.world.stats();
                }
                MainCommmand::Quit(u) => {
                    // println!("quit {}", u);
                    if u > 0 {
                        match &self.global.pending_load {
                            Some(l) => {
                                mutations.push((id, MainCommmand::Load(l.to_owned())));
                            }
                            _ => mutations.push((id, MainCommmand::Quit(u))),
                        }
                        self.global.pending_load = None;
                    } else {
                        self.global.state_changes.push(StateChange::Quit);
                    }
                    self.global.is_state_changed = true;
                }
                MainCommmand::LoopComplete(img_result) => {
                    match img_result {
                        Some((img, is_sky)) => {
                            let raster_id = if is_sky { 1 } else { 0 };

                            if !self.bundle_manager.is_single() {
                                self.bundle_manager.set_raster(id, raster_id, img);
                                mutations.push((id, MainCommmand::Meta(raster_id)));
                            } else {
                                self.gui.replace_image(img, is_sky);
                            }
                        }
                        _ => {}
                    }
                    // if let Some(reff) = completed_bundles.get_mut(&id) {
                    //     *reff += 1;
                    // } else {
                    //     completed_bundles.insert(id, 1);
                    // }
                    completed_bundles.insert(id, true);
                    loop_complete = true;
                }
                _ => {}
            };
        }

        if !mutations.is_empty() {
            let mut only_one_gui_sync = true;
            for (id, m) in mutations {
                match m {
                    MainCommmand::Reload() => crate::command::reload(self, id),
                    MainCommmand::Quit(u) => {
                        crate::command::hard_reset(self);
                        crate::command::load_empty(self);
                    }
                    MainCommmand::Subload(file, is_overlay) => {
                        crate::command::load(self, Some(file), None, None, Some((id, is_overlay)));
                    }
                    MainCommmand::Load(file) => {
                        crate::command::hard_reset(self);
                        println!("load {}", file);
                        crate::command::load(self, Some(file), None, None, None);
                    }
                    MainCommmand::Meta(d) => {
                        if only_one_gui_sync {
                            match self.bundle_manager.get_rasters(d) {
                                Some(rasters) => {
                                    self.gui.replace_image(rasters, d == 0);
                                }
                                None => {}
                            }
                            only_one_gui_sync = false;
                        }
                    }
                    _ => {}
                }
            }
        }
        let instance_buffers = if loop_complete {
            Some(self.ent_manager.check_ents(
                self.global.iteration,
                &self.device,
                &self.tex_manager,
                &self.model_manager,
            ))
        } else {
            // println!("skip frame");
            None
        };

        self.global.iteration += 1;
        instance_buffers
    }
    fn val2float(val: &ValueMap) -> f32 {
        match val {
            ValueMap::Float(f) => *f,
            ValueMap::Integer(i) => *i as f32,
            ValueMap::Bool(b) => {
                if *b {
                    1.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        }
    }
    fn val2bool(val: &ValueMap) -> bool {
        match val {
            ValueMap::Float(f) => *f != 0.0,
            ValueMap::Integer(i) => *i != 0,
            ValueMap::Bool(b) => *b,
            _ => false,
        }
    }
    fn val2array(val: &ValueMap) -> Vec<f32> {
        match val {
            ValueMap::Array(a) => a.iter().map(|v| Self::val2float(v)).collect::<Vec<f32>>(),
            ValueMap::Float(f) => vec![*f],
            _ => vec![],
        }
    }
    fn val2vec3(val: &ValueMap) -> [f32; 3] {
        match val {
            ValueMap::Array(a) => match a.len() {
                1 => [Self::val2float(&a[0]), 0., 0.],
                2 => [Self::val2float(&a[0]), Self::val2float(&a[1]), 0.],
                3 => [
                    Self::val2float(&a[0]),
                    Self::val2float(&a[1]),
                    Self::val2float(&a[2]),
                ],
                _ => [0., 0., 0.],
            },
            ValueMap::Float(f) => [*f, 0., 0.],
            _ => [0., 0., 0.],
        }
    }
    fn val2string(val: &ValueMap) -> Option<&String> {
        match val {
            ValueMap::String(s) => Some(s),
            _ => None,
        }
    }

    fn render(&mut self, instance_buffers: &InstanceBuffer) -> Result<(), wgpu::SurfaceError> {
        self.global.delayed += 1;
        if self.global.delayed >= 128 {
            self.global.delayed = 0;
            println!("fps::{}", self.global.fps);
        }
        // self.loop_helper.loop_start();

        let s = render::render_loop(self, self.global.iteration, instance_buffers);
        if let Some(fps) = self.loop_helper.report_rate() {
            self.global.fps = fps;
        }
        self.loop_helper.loop_sleep(); //DEV better way to sleep that allows maincommands to come through but pauses render?

        // match self.loop_helper.report_rate() {
        //     Some(t) => println!("now we over {}", t),
        //     None => println!("not yet"),
        // };
        s
    }

    pub fn toggle_fullscreen(&mut self) {
        self.global.fullscreen = !self.global.fullscreen;
        self.check_fullscreen();
    }

    fn check_fullscreen(&self) {
        if self.global.fullscreen != self.global.fullscreen_state {
            if self.global.fullscreen {
                // TODO windows;; macos use Fullscreen::Borderless
                self.win_ref
                    .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
            } else {
                self.win_ref.set_fullscreen(None)
            }
        }
    }
}

fn main() {
    crate::parse::test(&"test.lua".to_string());
    env_logger::init();
    let event_loop = EventLoop::new();
    let win = WindowBuilder::new()
        .with_inner_size(winit::dpi::LogicalSize::new(640i32, 480i32))
        .build(&event_loop)
        .unwrap();
    win.set_title("Petrichor");
    let center = winit::dpi::LogicalPosition::new(320.0f64, 240.0f64);
    let rwindow = Rc::new(win);

    // State::new uses async code, so we're going to wait for it to finish

    let mut core = pollster::block_on(Core::new(Rc::clone(&rwindow)));

    crate::command::load_empty(&mut core);

    core.loggy.clear();

    core.global.state_changes.push(StateChange::Config);
    // DEV a little delay trick to ensure any pending requests in our "console" app are completed before the following state change is made
    core.global.state_delay = 3;
    core.global.is_state_changed = true;
    let mut bits: ControlState = ([false; 256], [0.; 10]);

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
                let id = core.bundle_manager.console_bundle_target;
                crate::command::reload(&mut core, id);
            }

            #[cfg(not(feature = "include_auto"))]
            {
                // crate::command::load_empty(&mut core);
            }
        }
    }

    // :reload(core);
    let mut instance_buffers = vec![];
    let mut updated_bundles = FxHashMap::default();

    event_loop.run(move |event, _, control_flow| {
        core.loop_helper.loop_start();
        if !core.global.console {
            controls::bit_check(&event, &mut bits);
            bits.1[0] = core.global.mouse_pos.x;
            bits.1[1] = core.global.mouse_pos.y;
            bits.1[2] = core.global.mouse_delta.x;
            bits.1[3] = core.global.mouse_delta.y;
            bits.1[4] = core.global.mouse_buttons[0];
            bits.1[5] = core.global.mouse_buttons[1];
            bits.1[6] = core.global.mouse_buttons[2];
            bits.1[7] = core.global.cursor_projected_pos.x;
            bits.1[8] = core.global.cursor_projected_pos.y;
            bits.1[9] = core.global.cursor_projected_pos.z;
        } else if core.global.mouse_grabbed_state {
            rwindow.set_cursor_visible(true);
            rwindow.set_cursor_grab(CursorGrabMode::None);
            core.global.mouse_grabbed_state = false;
        }
        if core.global.is_state_changed {
            if core.global.state_delay > 0 {
                core.global.state_delay -= 1;
                // println!("delaying state change {} ", core.global.state_delay);
            } else {
                core.global.is_state_changed = false;
                let states: Vec<StateChange> = core.global.state_changes.drain(..).collect();
                for state in states {
                    match state {
                        // StateChange::Fullscreen => {core.check_fullscreen();
                        StateChange::MouseGrabOn => {
                            rwindow.set_cursor_visible(false);
                            rwindow.set_cursor_position(center).unwrap();
                            rwindow
                                .set_cursor_grab(CursorGrabMode::Confined)
                                .or_else(|_| rwindow.set_cursor_grab(CursorGrabMode::Locked));
                            core.global.mouse_grabbed_state = true;
                        }
                        StateChange::MouseGrabOff => {
                            rwindow.set_cursor_visible(true);
                            rwindow.set_cursor_grab(CursorGrabMode::None);
                            core.global.mouse_grabbed_state = false;
                        }
                        StateChange::Resized => {
                            core.debounced_resize();
                        }
                        StateChange::Quit => {
                            *control_flow = ControlFlow::Exit;
                        }
                        StateChange::Config => {
                            crate::asset::parse_config(
                                &mut core.global,
                                core.bundle_manager.get_lua(),
                                &mut core.loggy,
                            );

                            // core.config = crate::config::Config::new();
                            // core.config.load();
                            // core.config.apply(&mut core);
                        }
                    }
                }
                core.check_fullscreen();
            }
        }
        if core.input_manager.update(&event) {
            controls::controls_evaluate(&mut core, control_flow);
            // frame!("START");

            core.global.mouse_delta = vec2(0., 0.);
            // frame!("END");
            // frame!();
        }

        // Run our update and look for a "loop complete" return call from the bundle manager calling the lua loop in a previous step.
        // The lua context upon completing a loop will send a MainCommmand::LoopComplete to this thread.
        if let Some(buff) = core.update(&mut updated_bundles) {
            instance_buffers = buff;
        }
        core.bundle_manager.call_loop(&mut updated_bundles, bits);

        match core.render(&instance_buffers) {
            Ok(_) => {}
            // Reconfigure the surface if lost
            Err(wgpu::SurfaceError::Lost) => core.resize(core.size),
            // The system is out of memory, we should probably quit
            Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
            // All other errors (Outdated, Timeout) should be resolved by the next frame
            Err(e) => eprintln!("{:?}", e),
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
            Event::DeviceEvent { device_id, event } => match event {
                DeviceEvent::MouseMotion { delta } => {
                    core.global.mouse_delta = vec2(delta.0 as f32, delta.1 as f32);
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

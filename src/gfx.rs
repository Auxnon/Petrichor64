#[cfg(feature = "audio")]
use crate::sound::{self, SoundCommand};
use crate::{
    error_window, global::GuiParams, render, texture::TexManager,
};
use crate::{ent::EntityUniforms, global::GuiStyle, post::Post, texture::TexTuple};
use bytemuck::{Pod, Zeroable};
use glam::{vec2, vec3, Mat4};
#[cfg(feature = "audio")]
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::mem;
use wgpu::{util::DeviceExt, BindGroup, Buffer, CompositeAlphaMode, RenderPipeline, Texture};
use wgpu::{BackendOptions, ExperimentalFeatures, Features, Trace};
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    // platform::macos::WindowExtMacOS,
    window::Window,
};

const MAX_ENTS: u64 = 10000;

pub struct Gfx<'w> {
    pub uniform_buf: Buffer,
    pub uniform_alignment: u64,
    pub entity_bind_group: BindGroup,
    entity_uniform_buf: Buffer,
    pub main_bind_group: BindGroup,
    pub master_texture: Texture,
    pub post: Post,
    pub win_ref: Arc<Window>,
    pub main_layout: wgpu::BindGroupLayout,
    pub gui_aux_layout: wgpu::BindGroupLayout,
    pub render_pipeline: wgpu::RenderPipeline,
    pub surface: wgpu::Surface<'w>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub depth_texture: wgpu::TextureView,
    pub size: winit::dpi::PhysicalSize<u32>,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalUniforms {
    view: [[f32; 4]; 4],
    persp: [[f32; 4]; 4],
    adjustments: [[f32; 4]; 4],
    specs: [f32; 4],
    // L0 retro lighting: directional sun. `light_color.w` carries ambient.
    light_dir: [f32; 4],
    light_color: [f32; 4],
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
        view_formats: &[],
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        // 4.
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        compare: Some(wgpu::CompareFunction::LessEqual), // 5.
        lod_min_clamp: 0.0,
        lod_max_clamp: 100.0,
        ..Default::default()
    });

    let view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
    (depth_texture, view, sampler)
}

impl<'w> Gfx<'w> {
    pub async fn new(
        rwindow: Arc<Window>,
        tex_manager: &TexManager,
    ) -> (Self, RenderPipeline, RenderPipeline) {
        // crate::texture::save_audio_buffer(&vec![255u8; 1024]);
        // let b=Box::new(*rwindow);
        // let window = &*rwindow;
        // let ww= SurfaceTarget::Window(Box::new(*rwindow));
        // On the web the canvas often isn't laid out yet at init, so
        // inner_size() reports 0x0 — which makes the surface config and every
        // texture derived from it 0x0 and Dawn/WebGPU rejects them. Start from a
        // sane default; the first Resized event reconfigures to the real size.
        let size = {
            let s = rwindow.inner_size();
            if s.width == 0 || s.height == 0 {
                winit::dpi::PhysicalSize::new(640, 548)
            } else {
                s
            }
        };

        // The instance is a handle to our GPU
        // BackendBit::PRIMARY => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds {
                for_resource_creation: None,
                for_device_loss: None,
            },
            backend_options: BackendOptions::from_env_or_default(),
            flags: wgpu::InstanceFlags::empty(),
            display: None,
        });
        // let arc_window = std::sync::Arc::new(window);
        // arc_window.inn

        // wgpu::Backends::all());
        let surface = match instance.create_surface(rwindow.clone()) {
            Ok(surface) => surface,
            Err(e) => {
                error_window(Box::new(e));
                std::process::exit(1);
            }
        };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to request adapter");

        let (device, queue) = match adapter
            .request_device(&wgpu::DeviceDescriptor {
                trace: Trace::Off, // TODO make this on, where should it save?
                experimental_features: ExperimentalFeatures::disabled(),
                label: None,
                required_features: Features::empty(),
                required_limits: wgpu::Limits {
                    max_storage_textures_per_shader_stage: 8,
                    ..wgpu::Limits::default()
                },
                memory_hints: wgpu::MemoryHints::Performance, // TODO try setting this to manual, how much memory do we need?
            })
            .await
        {
            Ok((device, queue)) => (device, queue),
            Err(e) => {
                error_window(Box::new(e));
                std::process::exit(1);
            }
        };
        device.on_uncaptured_error(Arc::new(|e: wgpu::Error| {
            error_window(Box::new(e));
            std::process::exit(1);
        }));

        //this order is important, since models can load their own textures we need assets to init first

        let TexTuple {
            view: diffuse_texture_view,
            sampler: diffuse_sampler,
            texture: diff_tex,
        } = tex_manager.finalize(&device, &queue);

        let surface_caps = surface.get_capabilities(&adapter);
        // Prefer sRGB surface formats; fall back to first available
        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            desired_maximum_frame_latency: 1,
            format: surface_format,
            width: size.width,
            height: size.height,
            // present_mode: wgpu::PresentMode::Immediate, TODO used to be immediate, what have we
            // lost? can we check if immediate is better?
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: CompositeAlphaMode::Opaque,
            view_formats: vec![],
        };

        // TODO match
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

        let entity_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            layout: &entity_layout,
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

        let main_uniform_size = mem::size_of::<GlobalUniforms>() as wgpu::BufferAddress;

        let main_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("main bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(main_uniform_size), //wgpu::BufferSize::new(64),
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

        // let gui_uniform_size = mem::size_of::<GuiUniforms>() as wgpu::BufferAddress;
        let gui_aux_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gui secondary bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true }, //wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
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
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true }, //wgpu::TextureSampleType::Uint,
                        view_dimension: wgpu::TextureViewDimension::D2,
                    },
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
            light_dir: [0.0, 0.0, -1.0, 0.0],
            light_color: [0.0, 0.0, 0.0, 1.0], // fullbright: color 0 + ambient 1
        };

        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::bytes_of(&render_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[Some(&main_layout), Some(&entity_layout)],
                ..Default::default()
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None, // TODO will this save us startup time?
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: Default::default(),
                module: &shader,
                entry_point: Some("vs_main"),
                //targets:&[wgpu::],
                buffers: &[
                    crate::model::Vertex::desc(),
                    crate::ent::EntityUniforms::desc(),
                ], //&vertex_buffers, //,
            },

            fragment: Some(wgpu::FragmentState {
                module: &shader,
                compilation_options: Default::default(),
                entry_point: Some("fs_main"),
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
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less), // 1.
                stencil: wgpu::StencilState::default(),           // 2.
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
        });

        let depth = create_depth_texture(&config, &device);

        // let switch_board = Arc::new(RwLock::new(switch_board::SwitchBoard::new()));

        //Gui

        // Create main bind group
        let main_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &main_layout,
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

        // =================== GUI Pipeline ===================

        let gui_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Gui Render Pipeline Layout"),
            bind_group_layouts: &[Some(&main_layout), Some(&gui_aux_layout)],
            ..Default::default()
        });

        let gui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("Gui Pipeline"),
            layout: Some(&gui_pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: Default::default(),
                module: &shader,
                entry_point: Some("gui_vs_main"),
                buffers: &[], //&vertex_buffers, //,
            },

            fragment: Some(wgpu::FragmentState {
                compilation_options: Default::default(),
                module: &shader,
                entry_point: Some("gui_fs_main"),
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
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less), // 1.
                stencil: wgpu::StencilState::default(),           // 2.
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
        });

        // =================== Sky Pipeline ===================

        let sky_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Sky Render Pipeline Layout"),
            bind_group_layouts: &[Some(&main_layout)],
            ..Default::default()
        });

        let sky_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("Sky Pipeline"),
            layout: Some(&sky_pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: Default::default(),
                module: &shader,
                entry_point: Some("sky_vs_main"),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                compilation_options: Default::default(),
                module: &shader,
                entry_point: Some("sky_fs_main"),
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
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
        });

        let post = Post::new(
            &config,
            &device,
            &shader,
            &main_layout,
            &uniform_buf,
            // main_uniform_size,
        );

        (
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
                // switch_board: Arc::clone(&switch_board),
                post,
                main_bind_group,
                main_layout,
                gui_aux_layout,
                entity_bind_group,
                entity_uniform_buf,

                master_texture: diff_tex,
                win_ref: rwindow,
            },
            gui_pipeline,
            sky_pipeline,
        )
    }
    pub fn compute_gui_size(gui_params: &GuiParams, new_size: PhysicalSize<u32>) -> (u32, u32) {
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

    pub fn set_config_size(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        // Clamp to the device's max 2D texture dimension. On the web a canvas can
        // enter a client-size <-> backing-buffer×DPR feedback loop and request a
        // surface larger than the GPU allows, which aborts the wasm module
        // ("Texture size exceeded maximum"). Clamping keeps it alive; the canvas
        // CSS pin (attach_canvas_to_dom) is what actually stops the growth.
        let max = self.device.limits().max_texture_dimension_2d;
        let w = new_size.width.clamp(1, max);
        let h = new_size.height.clamp(1, max);
        self.size = winit::dpi::PhysicalSize::new(w, h);
        self.config.width = w;
        self.config.height = h;
    }

    pub fn resize(&mut self, gui_params: &GuiParams) -> (u32, u32) {
        let new_size = self.size;
        self.surface.configure(&self.device, &self.config);
        let d = create_depth_texture(&self.config, &self.device);
        self.depth_texture = d.1;
        self.post
            .resize(&self.device, new_size, &self.uniform_buf, &self.main_layout);
        Self::compute_gui_size(gui_params, new_size)
    }

    pub fn set_fullscreen(&self, enable: bool) {
        // TODO windows;; macos use Fullscreen::Borderless
        if enable {
            self.win_ref
                .set_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        } else {
            self.win_ref.set_fullscreen(None)
        }
    }

    pub fn set_window_size(&self, x: Option<&f32>, y: Option<&f32>) {
        let _ = self.win_ref.request_inner_size(LogicalSize::new(
            x.unwrap_or(&(self.size.width as f32))
                .clamp(10., f32::INFINITY) as u32,
            y.unwrap_or(&(self.size.height as f32))
                .clamp(10., f32::INFINITY) as u32,
        ));
    }

    pub fn set_title(&self, title: &str) {
        self.win_ref.set_title(title);
    }
}

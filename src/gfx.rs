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
    /// The shadow map (guide/shdw.md): a square depth target rendered from
    /// the lum{} light's POV, sampled by fs_main. Not resized with the window;
    /// `shdw{size=}` sets its edge length (see `set_shadow_size`).
    pub shadow_view: wgpu::TextureView,
    pub shadow_bind_group: wgpu::BindGroup,
    shadow_layout: wgpu::BindGroupLayout,
    shadow_sampler: wgpu::Sampler,
    /// Current edge length of the shadow map in texels.
    pub shadow_size: u32,
    pub shadow_pipeline: wgpu::RenderPipeline,
    /// Skinned-mesh variants of the two pipelines above (src/skeleton.rs).
    pub skinned_pipeline: wgpu::RenderPipeline,
    pub skinned_shadow_pipeline: wgpu::RenderPipeline,
    pub palette_layout: wgpu::BindGroupLayout,
    pub surface: wgpu::Surface<'w>,
    /// Kept so the surface can be rebuilt without tearing down the device, the
    /// pipelines, or the running game — Android destroys the native window when the
    /// app is backgrounded (screen sleep) and hands back a *new* one on resume, at
    /// which point the old surface is dead. See `recreate_surface`.
    instance: wgpu::Instance,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pub depth_texture: wgpu::TextureView,
    /// The window's (surface's) size in physical pixels: what the post pass
    /// draws, and the aspect ratio everything uses.
    pub size: winit::dpi::PhysicalSize<u32>,
    /// What the scene renders at (`post` and `depth_texture`): `crt_resolution`
    /// pixels wide, never wider than the window, at the window's aspect. The
    /// look is low-res anyway, and rendering every physical pixel made the
    /// cost grow with the display: 4x on a 2x (Retina) screen, 7x at 5K
    /// fullscreen, for detail the CRT pass throws away. See `internal_size`.
    pub render_size: winit::dpi::PhysicalSize<u32>,
    /// The gui drawn straight onto the window in the post pass (no depth), so
    /// with the LCD monitor it stays sharp at full resolution instead of being
    /// squeezed through the scene's internal resolution. The CRT monitor still
    /// draws it in the scene, so it curves and scanlines with everything else.
    pub gui_overlay_pipeline: wgpu::RenderPipeline,
    /// The `crt_resolution` that `render_size` was last derived from.
    render_res: f32,
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
    // L2 distance fog: rgb + w = far distance (w=0 disables).
    fog_color: [f32; 4],
    // L2 hemisphere ambient: sky rgb (w>0 enables) + ground rgb. Sun shape only.
    amb_sky: [f32; 4],
    amb_ground: [f32; 4],
    // lum{} shape state: xyz = world pos (cone/sphere), w = falloff range.
    light_pos: [f32; 4],
    // x = shape (0=sun,1=cone,2=sphere), y = cone half-angle (radians).
    light_shape: [f32; 4],
    // Light-space view*proj for the shadow map.
    light_view_proj: [[f32; 4]; 4],
    // x = shdw() on/off.
    shadow_on: [f32; 4],
    // Chip-owned render state: x = 3-point texture filtering (R43).
    chip: [f32; 4],
}
// pub const OPENGL_TO_WGPU_MATRIX: Mat4 = Mat4:new()
//     1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.5, 1.0,
// };

fn create_depth_texture(
    size: PhysicalSize<u32>,
    device: &wgpu::Device,
) -> (wgpu::Texture, wgpu::TextureView, wgpu::Sampler) {
    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width: size.width,
            height: size.height,
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
        // Chrome on Linux exposes `navigator.gpu` without a WebGPU adapter;
        // the detection drops WebGPU there so WebGL2 gets used instead.
        let instance = wgpu::util::new_instance_with_webgpu_detection(wgpu::InstanceDescriptor {
            backends: default_backends(),
            memory_budget_thresholds: wgpu::MemoryBudgetThresholds {
                for_resource_creation: None,
                for_device_loss: None,
            },
            backend_options: BackendOptions::from_env_or_default(),
            flags: wgpu::InstanceFlags::empty(),
            display: None,
        })
        .await;
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
                    // No shader uses storage textures today; capped at what the
                    // adapter offers so devices with fewer (e.g. SwiftShader's 4)
                    // still get a device instead of failing outright.
                    max_storage_textures_per_shader_stage: adapter
                        .limits()
                        .max_storage_textures_per_shader_stage
                        .min(8),
                    ..base_limits(&adapter)
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

        // CHOR_SNAP test grabs read the presented frame back, which needs
        // COPY_SRC on the swapchain; only asked for when grabbing.
        #[cfg(not(target_arch = "wasm32"))]
        let snap_usage = if crate::snap::enabled()
            && surface_caps.usages.contains(wgpu::TextureUsages::COPY_SRC)
        {
            wgpu::TextureUsages::COPY_SRC
        } else {
            wgpu::TextureUsages::empty()
        };
        #[cfg(target_arch = "wasm32")]
        let snap_usage = wgpu::TextureUsages::empty();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | snap_usage,
            desired_maximum_frame_latency: 1,
            format: surface_format,
            width: size.width,
            height: size.height,
            // present_mode: wgpu::PresentMode::Immediate, TODO used to be immediate, what have we
            // lost? can we check if immediate is better?
            present_mode: wgpu::PresentMode::Fifo,
            // Ask for what the surface actually supports. `Opaque` was hardcoded,
            // which is fine on desktop and the web but fatal on Android, where the
            // surface reports only `[Inherit]` — `Surface::configure` fails
            // validation and the app dies before drawing a frame. Opaque is still
            // preferred where it exists (the engine draws a full-screen opaque
            // image, and letting the compositor blend costs work for nothing).
            alpha_mode: if surface_caps
                .alpha_modes
                .contains(&CompositeAlphaMode::Opaque)
            {
                CompositeAlphaMode::Opaque
            } else {
                surface_caps.alpha_modes[0]
            },
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
            source: wgpu::ShaderSource::Wgsl(main_shader_source(&adapter).into()),
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
            0.,
        );

        let render_uniforms = GlobalUniforms {
            view: mx_view.to_cols_array_2d(),
            persp: mx_persp.to_cols_array_2d(),
            adjustments: Mat4::ZERO.to_cols_array_2d(),
            specs: [0.0, 0.0, 0.0, 0.0],
            light_dir: [0.0, 0.0, -1.0, 0.0],
            light_color: [0.0, 0.0, 0.0, 1.0], // fullbright: color 0 + ambient 1
            fog_color: [0.0, 0.0, 0.0, 0.0], // fog off
            amb_sky: [0.0, 0.0, 0.0, 0.0],   // w=0 => flat ambient
            amb_ground: [0.0, 0.0, 0.0, 0.0],
            light_pos: [0.0, 0.0, 0.0, 0.0],
            light_shape: [0.0, 0.0, 0.0, 0.0], // shape=0 => sun
            light_view_proj: Mat4::ZERO.to_cols_array_2d(),
            shadow_on: [0.0, 0.0, 0.0, 0.0],
            chip: [0.0, 0.0, 0.0, 0.0],
        };

        let uniform_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::bytes_of(&render_uniforms),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        // ============ Shadow map (guide/shdw.md) ============
        // A small, deliberately low-resolution depth target by default — not
        // tied to the window size; shdw{size=} rebuilds it — rendered from the
        // lum{} light's point of view (shadow_vs_main) and sampled with a
        // hardware comparison sampler in fs_main. Off by default (shdw());
        // when off the pass still runs (a handful of triangles into an
        // unsampled target) rather than branching pipeline setup itself.
        let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            compare: Some(wgpu::CompareFunction::LessEqual),
            lod_min_clamp: 0.0,
            lod_max_clamp: 1.0,
            ..Default::default()
        });
        let shadow_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow bind group layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
            ],
        });
        let shadow_size = SHADOW_MAP_SIZE;
        let (shadow_view, shadow_bind_group) =
            make_shadow_target(&device, &shadow_layout, &shadow_sampler, shadow_size);
        let shadow_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Shadow Pipeline Layout"),
                bind_group_layouts: &[Some(&main_layout)],
                ..Default::default()
            });
        let shadow_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("Shadow Pipeline"),
            layout: Some(&shadow_pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: Default::default(),
                module: &shader,
                entry_point: Some("shadow_vs_main"),
                buffers: &[
                    crate::model::Vertex::desc(),
                    crate::ent::EntityUniforms::desc(),
                ],
            },
            fragment: None,
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
                unclipped_depth: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState::default(),
                bias: SHADOW_DEPTH_BIAS,
            }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview_mask: None,
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[Some(&main_layout), Some(&entity_layout), Some(&shadow_layout)],
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

        let palette_layout = crate::skeleton::palette_layout(&device);
        let (skinned_pipeline, skinned_shadow_pipeline) = skinned_pipelines(
            &device,
            &shader,
            [&main_layout, &entity_layout, &shadow_layout, &palette_layout],
            config.format,
        );

        let depth = create_depth_texture(size, &device);

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

        let gui_overlay_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            cache: None,
            label: Some("Gui Overlay Pipeline"),
            layout: Some(&gui_pipeline_layout),
            vertex: wgpu::VertexState {
                compilation_options: Default::default(),
                module: &shader,
                entry_point: Some("gui_vs_main"),
                buffers: &[],
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
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            // the post pass has no depth attachment
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
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
                instance,
                device,
                queue,
                size,
                render_size: size,
                render_res: 0.,
                gui_overlay_pipeline,
                config,
                depth_texture: depth.1,
                uniform_buf,
                uniform_alignment,
                // view_matrix: mx_view,
                // perspective_matrix: mx_persp,
                render_pipeline,
                skinned_pipeline,
                skinned_shadow_pipeline,
                palette_layout,
                shadow_view,
                shadow_bind_group,
                shadow_layout,
                shadow_sampler,
                shadow_size,
                shadow_pipeline,
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

    /// Rebuild the surface against the window's *current* native handle, keeping the
    /// device, pipelines, textures and the running game intact.
    ///
    /// Android destroys the native window whenever the app leaves the foreground (a
    /// screen sleep is enough) and supplies a new one on resume. The old surface
    /// refers to the window that no longer exists, so every frame after that draws
    /// nowhere — the app comes back black. Only the surface is stale, which is why
    /// this exists instead of rebuilding `Core` and losing the game's state.
    pub fn recreate_surface(&mut self) -> bool {
        match self.instance.create_surface(self.win_ref.clone()) {
            Ok(surface) => {
                // The new window can be a different size (rotation, a fold opening),
                // so trust it over the config we were holding.
                let s = self.win_ref.inner_size();
                if s.width > 0 && s.height > 0 {
                    self.size = s;
                    self.config.width = s.width;
                    self.config.height = s.height;
                }
                surface.configure(&self.device, &self.config);
                self.surface = surface;
                self.rebuild_scene_targets();
                true
            }
            Err(e) => {
                // Not fatal: without a surface we simply don't draw, and the next
                // resume gets another go. Aborting here would kill a running game
                // over a transient window handle.
                ::log::error!("could not recreate the surface: {}", e);
                false
            }
        }
    }

    pub fn resize(&mut self, gui_params: &GuiParams) -> (u32, u32) {
        let new_size = self.size;
        self.surface.configure(&self.device, &self.config);
        self.rebuild_scene_targets();
        Self::compute_gui_size(gui_params, new_size)
    }

    /// The scene's resolution for a `crt_resolution` and a window size:
    /// `crt_resolution` wide (the CRT pass quantises to that many columns),
    /// capped at the window's width, at the window's aspect ratio.
    pub fn internal_size(crt_resolution: f32, window: PhysicalSize<u32>) -> PhysicalSize<u32> {
        let ww = window.width.max(1);
        let wh = window.height.max(1);
        let w = if crt_resolution.is_finite() && crt_resolution >= 1. {
            (crt_resolution.round() as u32).min(ww)
        } else {
            ww
        };
        let h = ((w as u64 * wh as u64 + ww as u64 / 2) / ww as u64).max(1) as u32;
        PhysicalSize::new(w, h)
    }

    /// Size the scene's targets for this `crt_resolution`; a no-op unless it
    /// changed (called every frame, as `attr`/`chip` can change it any time).
    /// Rebuild the shadow map at `size` texels square if it changed
    /// (`shdw{size=}`), clamped to what the device can hold. Returns the size
    /// actually in use.
    pub fn set_shadow_size(&mut self, size: u32) -> u32 {
        let max = self.device.limits().max_texture_dimension_2d;
        let size = size.clamp(SHADOW_MAP_MIN, max.min(SHADOW_MAP_MAX));
        if size != self.shadow_size {
            let (view, bind_group) = make_shadow_target(
                &self.device,
                &self.shadow_layout,
                &self.shadow_sampler,
                size,
            );
            self.shadow_view = view;
            self.shadow_bind_group = bind_group;
            self.shadow_size = size;
        }
        size
    }

    pub fn set_render_resolution(&mut self, crt_resolution: f32) {
        if crt_resolution != self.render_res {
            self.render_res = crt_resolution;
            self.rebuild_scene_targets();
        }
    }

    /// Re-make the scene's colour (`post`) and depth targets at `render_size`,
    /// recomputed from the window size and `render_res`.
    fn rebuild_scene_targets(&mut self) {
        let rs = Self::internal_size(self.render_res, self.size);
        if rs != self.render_size {
            let msg = format!(
                "scene renders at {}x{} for a {}x{} window",
                rs.width, rs.height, self.size.width, self.size.height
            );
            #[cfg(target_arch = "wasm32")]
            ::log::info!("{}", msg);
            #[cfg(not(target_arch = "wasm32"))]
            println!("{}", msg);
        }
        self.render_size = rs;
        let d = create_depth_texture(rs, &self.device);
        self.depth_texture = d.1;
        self.post
            .resize(&self.device, rs, &self.uniform_buf, &self.main_layout);
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

    /// `attr{size={w, h}}`, in logical points; a missing one keeps the current
    /// size. `self.size` is physical pixels, so it's converted first: as it
    /// was, on a 2x display the dimension left out doubled.
    pub fn set_window_size(&self, x: Option<&f32>, y: Option<&f32>) {
        let now: LogicalSize<f32> = self.size.to_logical(self.win_ref.scale_factor());
        let _ = self.win_ref.request_inner_size(LogicalSize::new(
            x.unwrap_or(&now.width).clamp(10., f32::INFINITY) as u32,
            y.unwrap_or(&now.height).clamp(10., f32::INFINITY) as u32,
        ));
    }

    pub fn set_title(&self, title: &str) {
        self.win_ref.set_title(title);
    }

    /// Swap the main scene/GUI texture sampler's filter mode and rebuild the one
    /// `main_bind_group` that every draw call reads — the whole engine has exactly
    /// one sampler for the master texture (see `chip.md`), so a chip change only
    /// costs this, not a pipeline rebuild. R00 (today's default) is
    /// `mag_nearest=true, min_nearest=false`; R43 wants both `false` (full
    /// bilinear, the characteristic N64 blur); R30 wants both `true` (PS1 had no
    /// texture filtering at all).
    pub fn set_filter_mode(&mut self, mag_nearest: bool, min_nearest: bool) {
        let filter = |nearest: bool| {
            if nearest {
                wgpu::FilterMode::Nearest
            } else {
                wgpu::FilterMode::Linear
            }
        };
        let view = self
            .master_texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: filter(mag_nearest),
            min_filter: filter(min_nearest),
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });
        self.main_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.main_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
            label: None,
        });
    }
}

/// The skinned render and shadow pipelines: the plain ones plus the skin
/// vertex buffer and the palette at group 3. Kept in step with the plain
/// descriptors in `Gfx::new` by hand.
/// Default edge length of the shadow map (guide/shdw.md), in texels. Small on
/// purpose: shadows are meant to read as blocky. `shdw{size=}` changes it
/// within SHADOW_MAP_MIN..=SHADOW_MAP_MAX.
pub const SHADOW_MAP_SIZE: u32 = 128;
pub const SHADOW_MAP_MIN: u32 = 16;
pub const SHADOW_MAP_MAX: u32 = 4096;

/// The shadow depth target and the bind group fs_main samples it through.
fn make_shadow_target(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    sampler: &wgpu::Sampler,
    size: u32,
) -> (wgpu::TextureView, wgpu::BindGroup) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        size: wgpu::Extent3d {
            width: size,
            height: size,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Depth32Float,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        label: Some("shadow depth"),
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("shadow bind group"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
        ],
    });
    (view, bind_group)
}

/// Hardware depth bias for the shadow passes. Each of the map's few texels
/// covers a lot of ground, so a surface's own depth varies across one texel by
/// more than any fixed offset, and a constant bias alone leaves it shadowing
/// itself in stripes (acne). The slope term grows the bias with how steeply a
/// surface faces away from the light; the constant covers the flat-on case.
const SHADOW_DEPTH_BIAS: wgpu::DepthBiasState = wgpu::DepthBiasState {
    constant: 1,
    slope_scale: 1.0,
    clamp: 0.0,
};

fn skinned_pipelines(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    [main, entity, shadow, palette]: [&wgpu::BindGroupLayout; 4],
    format: wgpu::TextureFormat,
) -> (wgpu::RenderPipeline, wgpu::RenderPipeline) {
    let buffers = [
        crate::model::Vertex::desc(),
        crate::ent::EntityUniforms::desc(),
        crate::skeleton::SkinVertex::desc(),
    ];
    let primitive = wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face: wgpu::FrontFace::Ccw,
        cull_mode: None,
        polygon_mode: wgpu::PolygonMode::Fill,
        conservative: false,
        unclipped_depth: false,
    };
    let depth = Some(wgpu::DepthStencilState {
        format: wgpu::TextureFormat::Depth32Float,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::Less),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    });
    let multisample = wgpu::MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
    };

    let render_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Skinned Pipeline Layout"),
        bind_group_layouts: &[Some(main), Some(entity), Some(shadow), Some(palette)],
        ..Default::default()
    });
    let render = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        cache: None,
        label: Some("Skinned Pipeline"),
        layout: Some(&render_layout),
        vertex: wgpu::VertexState {
            compilation_options: Default::default(),
            module: shader,
            entry_point: Some("vs_skinned"),
            buffers: &buffers,
        },
        fragment: Some(wgpu::FragmentState {
            module: shader,
            compilation_options: Default::default(),
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive,
        depth_stencil: depth.clone(),
        multisample,
        multiview_mask: None,
    });

    let shadow_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("Skinned Shadow Pipeline Layout"),
        bind_group_layouts: &[Some(main), None, None, Some(palette)],
        ..Default::default()
    });
    let shadow = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        cache: None,
        label: Some("Skinned Shadow Pipeline"),
        layout: Some(&shadow_layout),
        vertex: wgpu::VertexState {
            compilation_options: Default::default(),
            module: shader,
            entry_point: Some("shadow_vs_skinned"),
            buffers: &buffers,
        },
        fragment: None,
        primitive,
        depth_stencil: depth.map(|d| wgpu::DepthStencilState {
            bias: SHADOW_DEPTH_BIAS,
            ..d
        }),
        multisample,
        multiview_mask: None,
    });
    (render, shadow)
}

/// Every backend natively. On the web WebGPU when the page can actually use
/// it, else WebGL2: a browser can hand out a WebGPU canvas context while
/// hiding the `GPUCanvasContext` interface (insecure origins), which wgpu
/// can't recover from, so check up front.
fn default_backends() -> wgpu::Backends {
    #[cfg(target_arch = "wasm32")]
    {
        let global = js_sys::global();
        let secure = js_sys::Reflect::get(&global, &"isSecureContext".into())
            .map(|v| v.as_bool().unwrap_or(false))
            .unwrap_or(false);
        let has_ctx = js_sys::Reflect::has(&global, &"GPUCanvasContext".into()).unwrap_or(false);
        if secure && has_ctx {
            wgpu::Backends::BROWSER_WEBGPU | wgpu::Backends::GL
        } else {
            log::warn!("WebGPU unavailable (secure context: {secure}), using WebGL2");
            wgpu::Backends::GL
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    wgpu::Backends::all()
}

/// Limits to ask for: the defaults, or WebGL2's on the GL backend in a
/// browser, which can't meet them.
fn base_limits(adapter: &wgpu::Adapter) -> wgpu::Limits {
    if cfg!(target_arch = "wasm32") && adapter.get_info().backend == wgpu::Backend::Gl {
        wgpu::Limits::downlevel_webgl2_defaults().using_resolution(adapter.limits())
    } else {
        wgpu::Limits::default()
    }
}

/// The main shader. WebGL2's GLSL ES 3.0 has no `noperspective`, so there the
/// R30 affine UVs fall back to perspective-correct ones.
fn main_shader_source(adapter: &wgpu::Adapter) -> std::borrow::Cow<'static, str> {
    let src = include_str!("shaders/shader.wgsl");
    if cfg!(target_arch = "wasm32") && adapter.get_info().backend == wgpu::Backend::Gl {
        src.replace("@interpolate(linear) ", "").into()
    } else {
        src.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_scene_renders_crt_resolution_wide_at_the_windows_aspect() {
        let size = |w, h| PhysicalSize::new(w, h);
        // a 2x (Retina) 640x548 window, and 5K fullscreen: both 720 wide
        assert_eq!(Gfx::internal_size(720., size(1280, 1096)), size(720, 617));
        assert_eq!(Gfx::internal_size(720., size(5120, 2880)), size(720, 405));
        // never wider than the window
        assert_eq!(Gfx::internal_size(720., size(640, 548)), size(640, 548));
        // the chip presets
        assert_eq!(Gfx::internal_size(240., size(1920, 1080)), size(240, 135));
        // nonsense falls back to the window
        assert_eq!(Gfx::internal_size(0., size(800, 600)), size(800, 600));
        assert_eq!(Gfx::internal_size(f32::NAN, size(800, 600)), size(800, 600));
        // and nothing is ever zero
        assert_eq!(Gfx::internal_size(720., size(4000, 1)), size(720, 1));
    }
}

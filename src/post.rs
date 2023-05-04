use wgpu::{BindGroup, Buffer};
use winit::dpi::PhysicalSize;

const RENDER_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

pub struct Post {
    pub post_bind_group: BindGroup,
    pub post_pipeline: wgpu::RenderPipeline,
    pub post_texture_view: wgpu::TextureView,
    post_sampler: wgpu::Sampler,
    // uniform_buf: Buffer,
    pub post_texture: wgpu::Texture,
}

pub struct ScreenBinds {
    pub modernize: f32,
    pub dark_factor: f32,
    pub low_range: f32,
    pub high_range: f32,
    pub corner_harshness: f32,
    pub corner_ease: f32,
    pub crt_resolution: f32,
    pub glitchiness: [f32; 3],
    pub lumen_threshold: f32,
    pub fog: f32,
}

impl ScreenBinds {
    pub fn new() -> ScreenBinds {
        ScreenBinds {
            modernize: 1.,
            dark_factor: 0.4,
            low_range: 0.05,
            high_range: 0.6,
            corner_harshness: 1.0,
            corner_ease: 4.0,
            crt_resolution: 720.0, //320
            glitchiness: [0.12, 0., 0.02],
            lumen_threshold: 0.2,
            fog: 0.0,
        }
    }
}

impl Post {
    pub fn new(
        config: &wgpu::SurfaceConfiguration,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        main_layout: &wgpu::BindGroupLayout,
        uniform_buf: &Buffer,
    ) -> Post {
        let (post_texture_view, post_sampler, post_texture) =
            crate::texture::render_sampler(&device, (config.width, config.height));

        let (post_pipeline, post_bind_group_layout) = {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&main_layout],
                push_constant_ranges: &[],
            });
            (
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("PostProcess"),
                    layout: Some(&pipeline_layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "post_vs_main",
                        buffers: &[],
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: "post_fs_main",
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
                    // TODO why can't this be none?
                    depth_stencil: None,
                    // Some(wgpu::DepthStencilState {
                    //     format: wgpu::TextureFormat::Depth32Float,
                    //     depth_write_enabled: false,
                    //     depth_compare: wgpu::CompareFunction::Less, // 1.
                    //     stencil: wgpu::StencilState::default(),     // 2.
                    //     bias: wgpu::DepthBiasState::default(),
                    // }),
                    multisample: wgpu::MultisampleState::default(),
                    multiview: None,
                }),
                main_layout,
            )
        };

        // TODO this should be updated every time window resized i believe
        let post_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post bind group"),
            layout: &post_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&post_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&post_sampler),
                },
            ],
        });

        Post {
            post_bind_group,
            post_pipeline,

            post_texture_view,

            post_texture,
            post_sampler,
        }
    }

    pub fn resize(
        &mut self,
        device: &wgpu::Device,
        size: PhysicalSize<u32>,
        uniform_buf: &Buffer,
        main_group_layout: &wgpu::BindGroupLayout,
    ) {
        let (post_texture_view, post_sampler, post_texture) =
            crate::texture::render_sampler(device, (size.width, size.height));

        self.post_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("post bind group"),
            layout: main_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&post_texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&post_sampler),
                },
            ],
        });
        self.post_texture_view = post_texture_view;
        self.post_sampler = post_sampler;
        self.post_texture = post_texture;
    }
}

use wgpu::{BindGroup, Buffer, Surface};
use winit::dpi::PhysicalSize;

use crate::gui;

const RENDER_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Bgra8UnormSrgb;

pub struct Post {
    pub post_bind_group: BindGroup,
    pub post_bind_group_layout: wgpu::BindGroupLayout,
    pub post_pipeline: wgpu::RenderPipeline,
    pub post_texture_view: wgpu::TextureView,
    post_sampler: wgpu::Sampler,
    // uniform_buf: Buffer,
    pub post_texture: wgpu::Texture,
}

impl Post {
    pub fn new(
        config: &wgpu::SurfaceConfiguration,
        queue: &wgpu::Queue,
        device: &wgpu::Device,
        shader: &wgpu::ShaderModule,
        size: PhysicalSize<u32>,
        uniform_buf: &Buffer,
        uniform_size: u64,
    ) -> Post {
        // let (post_texture_view, post_sampler, post_texture, post_image) =
        //     gui::init_image(&device, &queue, size.width as f32 / size.height as f32);

        let (post_texture_view, post_sampler, post_texture) =
            crate::texture::render_sampler(&device, &queue);

        let (post_pipeline, post_bind_group_layout) = {
            let bind_group_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("post bind group layout"),
                    entries: &[
                        // TODO remove this
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
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
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
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&bind_group_layout],
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
                        // targets: &[RENDER_TARGET_FORMAT.into()],
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
                        }], // &[wgpu::ColorTargetState {
                            //     format: config.format,
                            //     blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                            //     write_mask: wgpu::ColorWrites::ALL,
                            // }],
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
                bind_group_layout,
            )
        };

        // wgpu::BindGroupLayoutEntry {
        //     binding: 0,
        //     visibility: wgpu::ShaderStages::VERTEX,
        //     ty: wgpu::BindingType::Buffer {
        //         ty: wgpu::BufferBindingType::Uniform,
        //         has_dynamic_offset: false,
        //         min_binding_size: wgpu::BufferSize::new(uniform_size), //wgpu::BufferSize::new(64),
        //     },
        //     count: None,

        // },

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
            post_bind_group_layout,
            post_texture_view,
            // uniform_buf,
            // post_image,
            post_texture,
            // post_texture_view,
            post_sampler,
        }
    }

    pub fn refresh(&mut self, device: &wgpu::Device, surface: Surface) {
        // let view = match surface.get_current_texture() {
        //     Ok(output) => output
        //         .texture
        //         .create_view(&wgpu::TextureViewDescriptor::default()),
        //     Err(e) => {
        //         println!("{:?}", e);
        //         panic!("");
        //     }
        // };

        // self.post_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        //     label: Some("post bind group"),
        //     layout: &self.post_bind_group_layout,
        //     entries: &[
        //         wgpu::BindGroupEntry {
        //             binding: 0,
        //             resource: self.uniform_buf.as_entire_binding(),
        //         },
        //         wgpu::BindGroupEntry {
        //             binding: 1,
        //             resource: wgpu::BindingResource::TextureView(&view),
        //         },
        //         wgpu::BindGroupEntry {
        //             binding: 2,
        //             resource: wgpu::BindingResource::Sampler(&self.post_sampler),
        //         },
        //     ],
        // });
    }
}

//#![windows_subsystem = "windows"]

use bytemuck::{Pod, Zeroable};
use ent_manager::EntManager;
use global::Global;
use itertools::Itertools;
use lua_define::LuaCore;
use once_cell::sync::OnceCell;
use std::{cell::RefCell, mem, sync::Arc};
use tile::World;
use tracy::frame;

use ent::Ent;
use glam::{vec2, vec3, Mat4};
use lazy_static::lazy_static;
use parking_lot::{FairMutex, Mutex, RwLock};

use switch_board::SwitchBoard;
use wgpu::{util::DeviceExt, BindGroup, Buffer, Texture};
use winit::{
    event::*,
    event_loop::{ControlFlow, EventLoop},
    // platform::macos::WindowExtMacOS,
    window::{Window, WindowBuilder},
};

use crate::ent::EntityUniforms;
use crate::gui::Gui;

mod asset;
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
mod ray;
mod render;
mod sound;
mod switch_board;
mod text;
mod texture;
mod tile;
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
    stream: cpal::Stream,
    view_matrix: Mat4,
    perspective_matrix: Mat4,
    uniform_buf: Buffer,
    uniform_alignment: u64,
    render_pipeline: wgpu::RenderPipeline,
    world: World,
    // ent_manager: EntManager,
    // vertex_buf: Rc<wgpu::Buffer>,
    // index_buf: Rc<wgpu::Buffer>,
    // index_count: usize,
    entity_bind_group: BindGroup,
    entity_uniform_buf: Buffer,
    bind_group: BindGroup,
    master_texture: Texture,
    gui: Gui,

    input_helper: winit_input_helper::WinitInputHelper,
    loop_helper: spin_sleep::LoopHelper,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GlobalUniforms {
    view: [[f32; 4]; 4],
    persp: [[f32; 4]; 4],
    time: [f32; 4],
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
lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    // pub static ref globals: Arc<RwLock<Global>> = Arc::new(RwLock::new(Global::new()));
    pub static ref lua_master : Arc<Mutex<OnceCell<LuaCore>>> = Arc::new((Mutex::new(OnceCell::new())));
    pub static ref ent_master : Arc<RwLock<OnceCell<EntManager>>> = Arc::new((RwLock::new(OnceCell::new())));
    // pub static ref ent_table: Arc<Mutex<Vec<lua_ent::LuaEnt>>>= Arc::new(Mutex::new(vec![]));
}

impl Core {
    async fn new(window: &Window) -> Core {
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

        let entity_uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: MAX_ENTS * uniform_alignment,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_format = wgpu::IndexFormat::Uint16;

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

        let (mx_view, mx_persp, mx_model) = render::generate_matrix(
            size.width as f32 / size.height as f32,
            0.,
            vec3(0., 0., 0.),
            vec2(0., 0.),
        );

        let render_uniforms = GlobalUniforms {
            view: mx_view.to_cols_array_2d(),
            persp: mx_persp.to_cols_array_2d(),
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

        let depth = create_depth_texture(&config, &device);

        let switch_board = Arc::new(RwLock::new(switch_board::SwitchBoard::new()));
        let dupe_switch = Arc::clone(&switch_board);

        //Gui

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

        let mut gui = Gui::new(gui_pipeline, gui_group, gui_texture, gui_image);
        gui.add_text("initialized".to_string());
        gui.add_img(&"map.tile.png".to_string());
        let world = World::new(&device);

        let mut loop_helper = spin_sleep::LoopHelper::builder()
            .report_interval_s(0.5) // report every half a second
            .build_with_target_rate(60.0); // limit to X FPS if possible
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
            global: Global::new(),
            switch_board,
            gui,
            bind_group,
            entity_bind_group,
            entity_uniform_buf,
            stream: sound::init_sound(dupe_switch).unwrap(),
            world,
            input_helper: winit_input_helper::WinitInputHelper::new(),
            master_texture: diff_tex,
            loop_helper,
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

    // #[allow(unused_variables)]
    // fn input(&mut self, event: &WindowEvent) -> bool {
    //     false
    // }

    fn update(&mut self) {
        match self.switch_board.try_read() {
            Some(r) => {
                if r.dirty {
                    drop(r);
                    let mut mutex = self.switch_board.write();

                    let make_count = mutex.make_queue.len();
                    if make_count > 0 {
                        for m in mutex.make_queue.drain(0..) {
                            if m.len() == 7 {
                                let m2 = vec![
                                    m[1].clone(),
                                    m[2].clone(),
                                    m[3].clone(),
                                    m[4].clone(),
                                    m[5].clone(),
                                    m[6].clone(),
                                ];
                                crate::model::edit_cube(m[0].clone(), m2, &self.device)

                                // let name = m[0].clone();
                                // println!("🧲 eyup1");

                                // match m[..].try_into() {
                                //     Ok(textures) => {
                                //         println!("🧲 eyup");

                                //         crate::model::edit_cube(name, textures, &self.device)
                                //     }
                                //     _ => {
                                //         log("cube creation missing variables, ignoring".to_string())
                                //     }
                                // }
                            }
                        }
                        mutex.make_queue.clear();
                    }

                    let tile_count = mutex.tile_queue.len();
                    if tile_count > 0 {
                        self.world.set_tile_from_buffer(&mutex.tile_queue);
                        self.world.build_dirty_chunks(&self.device);
                        mutex.tile_queue.clear();
                        println!("cooked {} tiles", tile_count);
                    }
                    mutex.dirty = false;
                }
            }
            None => {}
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let delta = self.loop_helper.loop_start();
        // or .loop_start_s() for f64 seconds
        if let Some(fps) = self.loop_helper.report_rate() {
            //  let current_fps = Some(fps);
            self.global.fps = fps;
        }

        let s = render::render_loop(self);
        self.loop_helper.loop_sleep();
        s
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    window.set_title("Petrichor");
    let s = window.inner_size();

    // window.set_fullscreen(Some(winit::window::Fullscreen::Borderless(
    //     window.current_monitor(),
    // )));

    // window.set_simple_fullscreen(true);
    // if false {
    // window.set_cursor_grab(true);
    // }

    // window.set_out(winit::dpi::LogicalSize::new(256, 256));
    // State::new uses async code, so we're going to wait for it to finish

    let mut core = pollster::block_on(Core::new(&window));

    let ent_guard = ent_master.read();
    let mut eman = EntManager::new();
    eman.uniform_alignment = core.uniform_alignment as u32;

    // eman.entities.push(Ent::new(
    //     vec3(0.0, 1.0, 0.0),
    //     45.,
    //     1.,
    //     0.,
    //     "chicken".to_string(),
    //     "plane".to_string(),
    //     core.uniform_alignment as u32,
    //     true,
    //     None,
    // ));

    ent_guard.get_or_init(|| eman);

    std::mem::drop(ent_guard);

    unsafe {
        tracy::startup_tracy();
    }

    event_loop.run(move |event, _, control_flow| {
        if core.input_helper.update(&event) {
            controls::controls_evaluate(&mut core, control_flow);
            frame!("START");
            core.update();

            match crate::lua_master.try_lock() {
                Some(cell) => match cell.get() {
                    Some(lu) => lu.call_loop(),
                    _ => {}
                },
                _ => {}
            }

            match core.render() {
                Ok(_) => {}
                // Reconfigure the surface if lost
                Err(wgpu::SurfaceError::Lost) => core.resize(core.size),
                // The system is out of memory, we should probably quit
                Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                // All other errors (Outdated, Timeout) should be resolved by the next frame
                Err(e) => eprintln!("{:?}", e),
            }
            frame!("END");
            frame!();
        }

        //     Event::RedrawRequested(_) => {
        //         println!("redraw entered");
        //         core.update();
        //         match core.render() {
        //             Ok(_) => {}
        //             // Reconfigure the surface if lost
        //             Err(wgpu::SurfaceError::Lost) => core.resize(core.size),
        //             // The system is out of memory, we should probably quit
        //             Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
        //             // All other errors (Outdated, Timeout) should be resolved by the next frame
        //             Err(e) => eprintln!("{:?}", e),
        //         }
        //     }
    });

    unsafe {
        tracy::shutdown_tracy();
    }
}

fn log(str: String) {
    crate::log::log(format!("main::{}", str));
}

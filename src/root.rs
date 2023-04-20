// #![windows_subsystem = "console"]
#![windows_subsystem = "windows"]
// #![allow(warnings)]
use crate::{
    bundle::BundleManager,
    ent_manager::{EntManager, InstanceBuffer},
    gfx::Gfx,
    global::{Global, GuiParams, StateChange},
    gui::ScreenIndex,
    lua_define::MainPacket,
    model::ModelManager,
    render,
    sound::{self, SoundCommand},
    texture::TexManager,
    types::ValueMap,
};
use bytemuck::{Pod, Zeroable};
use itertools::Itertools;
use rustc_hash::FxHashMap;
#[cfg(feature = "audio")]
use std::{
    mem,
    rc::Rc,
    sync::mpsc::{channel, Receiver, Sender},
};
// use tracy::frame;
use crate::{ent::EntityUniforms, global::GuiStyle, post::Post, texture::TexTuple, world::World};
use crate::{gui::Gui, log::LogType};
use glam::{vec2, vec3, Mat4};
use wgpu::{util::DeviceExt, BindGroup, Buffer, CompositeAlphaMode, Texture};
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event::*,
    event_loop::{ControlFlow, EventLoop},
    // platform::macos::WindowExtMacOS,
    window::{CursorGrabMode, Window, WindowBuilder},
};

/** All centralized engines and factories to be passed around in the main thread */
pub struct Core {
    pub global: Global,
    /** despite it's unuse, this stream needs to persist or sound will not occur */
    #[cfg(feature = "audio")]
    _stream: Option<cpal::Stream>,
    #[cfg(feature = "audio")]
    pub singer: Sender<SoundCommand>,

    pub world: World,
    pub pitcher: Sender<MainPacket>,
    pub catcher: Receiver<MainPacket>,

    pub gui: Gui,
    pub gfx: Gfx,

    pub loop_helper: spin_sleep::LoopHelper,
    pub tex_manager: TexManager,
    pub model_manager: ModelManager,
    pub ent_manager: EntManager,
    pub bundle_manager: BundleManager,

    pub loggy: crate::log::Loggy,

    pub input_manager: winit_input_helper::WinitInputHelper,
}

//DEV consider atomics such as AtomicU8 for switch_board or lazy static primatives

impl Core {
    pub async fn new(rwindow: Rc<Window>) -> Self {
        let tex_manager = crate::texture::TexManager::new();
        let (gfx, gui_pipeline, sky_pipeline) = Gfx::new(rwindow, &tex_manager).await;
        let model_manager = ModelManager::init(&gfx.device);
        let mut ent_manager = EntManager::new(&gfx.device);
        let global = Global::new();
        let mut loggy = crate::log::Loggy::new();
        let psize = winit::dpi::PhysicalSize::new(1280, 960);
        let gui_scaled = Gfx::compute_gui_size(&global.gui_params, psize);
        println!("gui_scaled: {:?}", gui_scaled);
        let mut gui = Gui::new(&gfx, gui_pipeline, sky_pipeline, gui_scaled, &mut loggy);

        let (w, h) = gui.get_console_size();
        loggy.set_dimensions(w, h);
        gui.add_text("initialized".to_string());

        if global.console {
            gui.enable_console(&loggy)
        }

        let world = World::new(loggy.make_sender());
        let loop_helper = spin_sleep::LoopHelper::builder()
            .report_interval_s(0.5) // report every half a second
            .build_with_target_rate(60.0); // limit to X FPS if possible

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
        ent_manager.uniform_alignment = gfx.uniform_alignment as u32;
        let input_manager = winit_input_helper::WinitInputHelper::new();
        let (pitcher, catcher) = channel::<MainPacket>();

        Self {
            global,
            #[cfg(feature = "audio")]
            _stream: stream_result,
            #[cfg(feature = "audio")]
            singer,
            world,
            pitcher,
            catcher,
            gui,
            loop_helper,
            tex_manager,
            model_manager,
            ent_manager,
            bundle_manager: BundleManager::new(),
            loggy,
            input_manager,
            gfx,
        }
    }
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.gfx.set_config_size(new_size);

            if self.global.state_delay == 0 {
                self.global.state_changes.push(StateChange::Resized);
            }
            self.global.state_delay = 15;
            self.global.is_state_changed = true;
        }
    }

    pub fn debounced_resize(&mut self) {
        let gui_scaled = self.gfx.resize(&self.global.gui_params);
        self.gui.resize(gui_scaled, &self.gfx);
        let (con_w, con_h) = self.gui.get_console_size();
        self.loggy.set_dimensions(con_w, con_h);
        self.bundle_manager.resize(gui_scaled.0, gui_scaled.1);
    }

    pub fn val2float(val: &ValueMap) -> f32 {
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
    pub fn val2bool(val: &ValueMap) -> bool {
        match val {
            ValueMap::Float(f) => *f != 0.0,
            ValueMap::Integer(i) => *i != 0,
            ValueMap::Bool(b) => *b,
            _ => false,
        }
    }
    pub fn val2array(val: &ValueMap) -> Vec<f32> {
        match val {
            ValueMap::Array(a) => a.iter().map(|v| Self::val2float(v)).collect::<Vec<f32>>(),
            ValueMap::Float(f) => vec![*f],
            _ => vec![],
        }
    }
    pub fn val2vec3(val: &ValueMap) -> [f32; 3] {
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
    pub fn val2string(val: &ValueMap) -> Option<&String> {
        match val {
            ValueMap::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn render(&mut self, instance_buffers: &InstanceBuffer) -> Result<(), wgpu::SurfaceError> {
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
        // self.loop_helper.loop_sleep(); //DEV better way to sleep that allows maincommands to come through but pauses render?
        s
    }

    pub fn toggle_fullscreen(&mut self) {
        self.global.fullscreen = !self.global.fullscreen;
        self.check_fullscreen();
    }

    pub fn check_fullscreen(&self) {
        if self.global.fullscreen != self.global.fullscreen_state {
            self.gfx.set_fullscreen(self.global.fullscreen);
        }
    }
}

#[cfg(feature = "audio")]
use crate::sound::{self, SoundCommand};
#[cfg(all(feature = "midi", not(target_arch = "wasm32")))]
use crate::midi;
use crate::{
    bundle::BundleManager,
    ent_manager::EntManager,
    global::Global,
    lua_define::MainPacket,
    model::ModelManager,
    texture::TexManager,
    types::ValueMap,
};
#[cfg(feature = "headed")]
use crate::{
    ent_manager::InstanceBuffer,
    gfx::Gfx,
    global::StateChange,
    render::{self, DrawState},
};

use std::sync::mpsc::{channel, Receiver, Sender};
#[cfg(feature = "headed")]
use std::sync::Arc;

// use tracy::frame;
use crate::world::World;
use crate::{gui::Gui, log::LogType};
#[cfg(feature = "headed")]
use colored::Colorize;
use rustc_hash::FxHashMap;
#[cfg(feature = "headed")]
use winit::window::Window;

#[cfg(feature = "headed")]
type IB = InstanceBuffer;
#[cfg(not(feature = "headed"))]
type IB = ();

/** All centralized engines and factories to be passed around in the main thread */
pub struct Core {
    pub global: Global,
    /** despite it's unuse, this stream needs to persist or sound will not occur */
    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    _stream: Option<cpal::Stream>,
    /// Live microphone capture, if one is in flight. Held only to keep the input
    /// stream open; dropped as soon as `mic_done` flips, which releases the mic
    /// (and clears the OS "in use" indicator) rather than holding it open.
    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    pub mic_stream: Option<cpal::Stream>,
    #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
    pub mic_done: Arc<std::sync::atomic::AtomicBool>,
    /// Web audio output (wasm): schedules PCM chunks on the AudioContext clock.
    /// Pumped once per frame; replaces the native cpal stream.
    #[cfg(all(feature = "audio", target_arch = "wasm32"))]
    pub web_audio: Option<crate::sound::WebOut>,
    #[cfg(feature = "audio")]
    pub singer: Sender<SoundCommand>,

    pub world: World,
    pub pitcher: Sender<MainPacket>,

    pub gui: Gui,
    #[cfg(feature = "headed")]
    pub gfx: Gfx<'static>,
    /// Headless input: a background thread feeds stdin lines here.
    #[cfg(not(feature = "headed"))]
    pub cli_thread_receiver: Receiver<String>,

    // spin_sleep uses std::time internally, which panics on wasm. On the web,
    // frame pacing comes from requestAnimationFrame + ControlFlow::WaitUntil, so
    // the LoopHelper is simply absent there.
    #[cfg(not(target_arch = "wasm32"))]
    pub loop_helper: spin_sleep::LoopHelper,
    pub tex_manager: TexManager,
    pub model_manager: ModelManager,
    pub ent_manager: EntManager,
    pub bundle_manager: BundleManager,

    pub completed_bundles: FxHashMap<u8, bool>,

    pub loggy: crate::log::Loggy,

    pub instance_buffers: IB,
}

//DEV consider atomics such as AtomicU8 for switch_board or lazy static primatives

impl<'core> Core {
    #[cfg(feature = "headed")]
    pub async fn new(rwindow: Arc<Window>) -> (Self, Receiver<MainPacket>) {
        let tex_manager = crate::texture::TexManager::new();
        let (gfx, gui_pipeline, sky_pipeline) = Gfx::new(rwindow, &tex_manager).await;
        println!("{}", "== begin ==".on_green());
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
        #[cfg(not(target_arch = "wasm32"))]
        let loop_helper = spin_sleep::LoopHelper::builder()
            .report_interval_s(0.5) // report every half a second
            .build_with_target_rate(60.0); // limit to X FPS if possible

        // Native: a cpal output stream (own audio thread). Web: cpal's WebAudio
        // backend crackles on the main thread, so we drive Web Audio ourselves
        // (an AudioWorklet, or a scheduled-buffer fallback). Both share the same synth.
        #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
        let (stream, singer) = sound::init();
        #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
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
        #[cfg(all(feature = "audio", target_arch = "wasm32"))]
        let (web_audio_res, singer) = sound::init_web();
        #[cfg(all(feature = "audio", target_arch = "wasm32"))]
        let web_audio = match web_audio_res {
            Ok(w) => Some(w),
            Err(e) => {
                loggy.log(
                    LogType::CoreError,
                    &format!("web audio init failed, continuing in silence!: {:?}", e),
                );
                None
            }
        };
        // MIDI in: grab the first available input port at boot so a plugged-in
        // (or OS-paired Bluetooth) controller just plays the synth. No device is
        // the normal case, so a failure here is logged at most, never fatal. The
        // connection lives in `midi`'s statics, so there's nothing to store.
        #[cfg(all(feature = "midi", not(target_arch = "wasm32")))]
        match midi::open(singer.clone(), None) {
            Ok(port) => loggy.log(LogType::Config, &format!("midi in: {}", port)),
            Err(e) => loggy.log(LogType::Config, &format!("no midi in ({})", e)),
        }

        ent_manager.uniform_alignment = gfx.uniform_alignment as u32;

        let (pitcher, catcher) = channel::<MainPacket>();
        let core = Self {
            global,
            #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
            _stream: stream_result,
            #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
            mic_stream: None,
            #[cfg(all(feature = "audio", not(target_arch = "wasm32")))]
            mic_done: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            #[cfg(all(feature = "audio", target_arch = "wasm32"))]
            web_audio,
            #[cfg(feature = "audio")]
            singer,
            world,
            pitcher,
            gui,
            #[cfg(not(target_arch = "wasm32"))]
            loop_helper,
            tex_manager,
            model_manager,
            ent_manager,
            bundle_manager: BundleManager::new(),
            loggy,
            gfx,
            completed_bundles: FxHashMap::default(),
            #[cfg(feature = "headed")]
            instance_buffers: vec![],
            #[cfg(not(feature = "headed"))]
            instance_buffers: (),
        };
        (core, catcher)
    }

    /// Headless Core: no GPU/window. Builds the same managers without a Gfx and
    /// spins a stdin reader thread for CLI input.
    #[cfg(not(feature = "headed"))]
    pub async fn new() -> (Self, Receiver<MainPacket>) {
        let tex_manager = crate::texture::TexManager::new();
        let model_manager = ModelManager::init();
        let ent_manager = EntManager::new();
        let global = Global::new();
        let mut loggy = crate::log::Loggy::new();
        let mut gui = Gui::new((256, 256), &mut loggy);

        let (w, h) = gui.get_console_size();
        loggy.set_dimensions(w, h);
        gui.add_text("initialized".to_string());

        let world = World::new(loggy.make_sender());
        let loop_helper = spin_sleep::LoopHelper::builder()
            .report_interval_s(0.5)
            .build_with_target_rate(60.0);

        let (_cli_thread_sender, cli_thread_receiver) = channel::<String>();
        // The TUI backend puts the terminal in raw mode and reads stdin itself
        // via crossterm; a second blocking line-reader thread on the same fd
        // would race it for bytes. Just leave the sender unused there so
        // `cli_thread_receiver.try_recv()` harmlessly never yields anything.
        #[cfg(not(feature = "render-tui"))]
        std::thread::spawn(move || loop {
            let mut line = String::new();
            match std::io::stdin().read_line(&mut line) {
                // 0 bytes == EOF (stdin closed / piped input exhausted): stop
                // reading instead of spinning on empty lines forever.
                Ok(0) | Err(_) => break,
                Ok(_) => {
                    if cli_thread_sender.send(line).is_err() {
                        break;
                    }
                }
            }
        });

        let (pitcher, catcher) = channel::<MainPacket>();
        let core = Self {
            global,
            #[cfg(feature = "audio")]
            _stream: None,
            #[cfg(feature = "audio")]
            singer: channel().0,
            world,
            pitcher,
            gui,
            cli_thread_receiver,
            loop_helper,
            tex_manager,
            model_manager,
            ent_manager,
            bundle_manager: BundleManager::new(),
            loggy,
            completed_bundles: FxHashMap::default(),
            instance_buffers: (),
        };
        (core, catcher)
    }

    #[cfg(feature = "headed")]
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

    #[cfg(feature = "headed")]
    pub fn debounced_resize(&mut self) {
        let gui_scaled = self.gfx.resize(&self.global.gui_params);
        self.gui
            .resize(&mut self.bundle_manager, gui_scaled, &self.gfx);
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

    #[cfg(feature = "headed")]
    pub fn render(&mut self) -> DrawState {
        self.global.delayed += 1;
        if self.global.delayed >= 128 {
            self.global.delayed = 0;
            println!("fps::{}", self.global.fps);
        }
        // self.loop_helper.loop_start();

        let res = render::render_loop(self, self.global.iteration);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(fps) = self.loop_helper.report_rate() {
            self.global.fps = fps;
        }
        res
        // self.loop_helper.loop_sleep(); //DEV better way to sleep that allows maincommands to come through but pauses render?
    }

    pub fn toggle_fullscreen(&mut self) {
        self.global.fullscreen = !self.global.fullscreen;
        #[cfg(feature = "headed")]
        self.check_fullscreen();
    }

    #[cfg(feature = "headed")]
    pub fn check_fullscreen(&self) {
        if self.global.fullscreen != self.global.fullscreen_state {
            self.gfx.set_fullscreen(self.global.fullscreen);
        }
    }

    pub fn send_notification(&mut self, msg: &str) {
        self.gui.push_notif(msg);
    }

    pub fn log(&mut self, kind: LogType, msg: &str) {
        self.loggy.log(kind, msg);
        self.send_notification(msg);
    }

    pub fn log_channel_error(&mut self) {
        self.log(LogType::LuaSysError, "!!Channel error");
    }

    pub fn log_check<T>(&mut self, s: Result<(), std::sync::mpsc::SendError<T>>)
    where
        T: Send + 'static,
    {
        if let Err(_) = s {
            self.log_channel_error();
        }
    }

    // pub fn update_raster(&mut self, d: ScreenIndex) {
    //     if let Some(rasters) = self.bundle_manager.get_rasters(match d {
    //         ScreenIndex::Primary => 0,
    //         _ => 1,
    //     }) {
    //         self.gui.replace_image(rasters, d);
    //     }
    // }
}

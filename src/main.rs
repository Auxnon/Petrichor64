// #![windows_subsystem = "console"]
#![windows_subsystem = "windows"]
// #![allow(warnings)]
use std::{
    env,
    rc::Rc,
    sync::mpsc::{channel, Receiver},
};

use crate::log::LogType;
use clipboard::{ClipboardContext, ClipboardProvider};
#[cfg(feature = "headed")]
use ent_manager::InstanceBuffer;
use glam::vec2;
use global::StateChange;
use gui::ScreenIndex;
use image::GenericImageView;
use itertools::Itertools;
use lua_define::{LuaResponse, MainPacket};
#[cfg(feature = "headed")]
use root::Core;
#[cfg(not(feature = "headed"))]
use root_headless::Core;
use rustc_hash::FxHashMap;
use types::{ControlState, GlobalMap};

mod asset;
mod bundle;
mod command;
#[cfg(feature = "headed")]
mod controls;
#[cfg(feature = "headed")]
mod ent;
mod ent_manager;
mod error;
mod file_util;
#[cfg(feature = "headed")]
mod gfx;
mod global;
mod gui;
mod log;
#[cfg(feature = "online_capable")]
mod lua_connection;
mod lua_define;
mod lua_ent;
mod lua_img;
mod model;
#[cfg(feature = "online_capable")]
mod online;
mod packet;
mod pad;
mod parse;
#[cfg(feature = "headed")]
mod post;
#[cfg(feature = "headed")]
mod ray;
#[cfg(feature = "headed")]
mod render;
#[cfg(feature = "headed")]
mod root;
#[cfg(not(feature = "headed"))]
mod root_headless;
#[cfg(feature = "audio")]
mod sound;
mod template;
#[cfg(feature = "headed")]
mod texture;
mod tile;
mod types;
mod world;

use command::MainCommmand;
#[cfg(feature = "headed")]
use winit::{
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{CursorGrabMode, WindowBuilder},
};

#[cfg(target_os = "windows")]
const OS: &str = "win";

#[cfg(target_os = "linux")]
const OS: &str = "nix";

#[cfg(target_os = "macos")]
const OS: &str = "mac";

fn main() {
    // crate::parse::test(&"test.lua".to_string());
    env_logger::init();

    let (pitcher, mut catcher) = channel::<MainPacket>();
    #[cfg(feature = "headed")]
    let (mut core, mut rwin, center, event_loop) = {
        let event_loop = EventLoop::new();
        let window_icon = {
            let icon =
                image::load_from_memory(include_bytes!("../assets/petrichor-small-icon.png"))
                    .expect("failed to load icon.png");
            let rgba = icon.as_rgba8().unwrap();
            let (width, height) = icon.dimensions();
            let rgba = rgba
                .chunks_exact(4)
                .flat_map(|rgba| rgba.iter().cloned())
                .collect::<Vec<_>>();
            winit::window::Icon::from_rgba(rgba, width, height).unwrap()
        };

        let win = match WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize::new(640i32, 548i32))
            .with_window_icon(Some(window_icon))
            .build(&event_loop)
        {
            Ok(win) => win,
            Err(e) => {
                error_window(Box::new(e));
                // println!("Error: {}", e);
                return;
            }
        };

        win.set_title("Petrichor64");

        let center = winit::dpi::LogicalPosition::new(320.0f64, 240.0f64);
        let rwindow = Rc::new(win);

        // State::new uses async code, so we're going to wait for it to finish
        (
            pollster::block_on(Core::new(Rc::clone(&rwindow), pitcher)),
            rwindow,
            center,
            event_loop,
        )
    };

    #[cfg(not(feature = "headed"))]
    let mut core = pollster::block_on(Core::new(pitcher));

    crate::command::load_empty(&mut core);

    core.loggy.clear();

    core.global.state_changes.push(StateChange::Config);
    // DEV a little delay trick to ensure any pending requests in our "console" app are completed before the following state change is made
    core.global.state_delay = 8;
    core.global.is_state_changed = true;
    let mut bits: ControlState = ([false; 256], [0.; 11]);

    let mut insta_load = |s: String| {
        core.global.console = false;
        #[cfg(feature = "headed")]
        core.gui.disable_console();
        // crate::command::hard_reset(&mut core);

        core.global.pending_load = Some(s.clone());
        // crate::command::load_from_string(&mut core, Some(s));
        core.bundle_manager.get_lua().call_drop(s);
    };

    if env::args().count() > 1 {
        let s = env::args().nth(1).unwrap();
        native_dialog::MessageDialog::new()
            .set_type(native_dialog::MessageType::Info)
            .set_title("Petrichor64 Info")
            .set_text(&s)
            .show_alert()
            .unwrap();
        insta_load(s);
    } else {
        match crate::asset::check_for_auto() {
            Some(s) => {
                insta_load(s);
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
                    #[cfg(not(feature = "studio"))]
                    core.gui.disable_console();
                    // crate::command::load_empty(&mut core);
                }
            }
        }
    }

    #[cfg(not(feature = "headed"))]
    type Param1 = ();
    #[cfg(not(feature = "headed"))]
    type Param2 = ();
    #[cfg(feature = "headed")]
    type Param1 = ControlFlow;
    #[cfg(feature = "headed")]
    type Param2 = Rc<winit::window::Window>;

    let state_change_check =
        move |c: &mut Core, control_flow: &mut Param1, rwindow: &mut Param2| {
            if c.global.is_state_changed {
                if c.global.state_delay > 0 {
                    c.global.state_delay -= 1;
                    // println!("delaying state change {} ", core.global.state_delay);
                } else {
                    c.global.is_state_changed = false;
                    let states: Vec<StateChange> = c.global.state_changes.drain(..).collect();
                    for state in states {
                        match state {
                            // StateChange::Fullscreen => {core.check_fullscreen();
                            #[cfg(feature = "headed")]
                            StateChange::MouseGrabOn => {
                                rwindow.set_cursor_visible(false);
                                rwindow.set_cursor_position(center).unwrap();
                                rwindow
                                    .set_cursor_grab(CursorGrabMode::Confined)
                                    .or_else(|_| rwindow.set_cursor_grab(CursorGrabMode::Locked));
                                c.global.mouse_grabbed_state = true;
                            }
                            #[cfg(feature = "headed")]
                            StateChange::MouseGrabOff => {
                                rwindow.set_cursor_visible(true);
                                rwindow.set_cursor_grab(CursorGrabMode::None);
                                c.global.mouse_grabbed_state = false;
                            }
                            #[cfg(feature = "headed")]
                            StateChange::Resized => {
                                c.debounced_resize();
                            }
                            #[cfg(feature = "headed")]
                            StateChange::Quit => {
                                *control_flow = ControlFlow::Exit;
                            }
                            #[cfg(not(feature = "headed"))]
                            StateChange::Quit => {
                                return true;
                            }
                            StateChange::Config => {
                                let res = crate::asset::parse_config(
                                    &mut c.global,
                                    c.bundle_manager.get_lua(),
                                    &mut c.loggy,
                                );
                                if let Some(s) = res {
                                    crate::command::run_con_sys(c, &s);
                                }

                                // as a bonus also check command line arguments here
                                let args: Vec<String> = std::env::args().collect();
                                if args.len() > 1 {
                                    // println!("cli args: {:?}", args);
                                    let mut command = None;
                                    for (i, arg) in args.iter().enumerate() {
                                        if arg.starts_with("-") {
                                            command = Some(arg.to_lowercase());
                                        } else if command.is_some() {
                                            match command.unwrap().as_str() {
                                                "--init" | "-i" => {
                                                    println!("cli-init: {:?}", arg);
                                                    crate::command::run_con_sys(c, &arg);
                                                }
                                                _ => {}
                                            }
                                            command = None;
                                        }
                                    }
                                }

                                // core.config = crate::config::Config::new();
                                // core.config.load();
                                // core.config.apply(&mut core);
                            }
                            StateChange::ModelChange(id) => {
                                #[cfg(feature = "headed")]
                                c.ent_manager.check_for_model_change(&c.model_manager, &id);
                            }
                        }
                    }
                    #[cfg(feature = "headed")]
                    c.check_fullscreen();
                }
            }
            false
        };

    #[cfg(feature = "headed")]
    {
        // :reload(core);
        let mut instance_buffers = vec![];
        let mut updated_bundles = FxHashMap::default();

        event_loop.run(move |event, _, control_flow| {
            // core.loop_helper.loop_start();
            if !core.global.console {
                controls::bit_check(&event, &mut bits);
                bits.1[0] = core.global.mouse_pos.x;
                bits.1[1] = core.global.mouse_pos.y;
                bits.1[2] = core.global.mouse_delta.x;
                bits.1[3] = core.global.mouse_delta.y;
                bits.1[4] = core.global.mouse_buttons[0];
                bits.1[5] = core.global.mouse_buttons[1];
                bits.1[6] = core.global.mouse_buttons[2];
                bits.1[7] = core.global.scroll_delta;
                bits.1[8] = core.global.cursor_projected_pos.x;
                bits.1[9] = core.global.cursor_projected_pos.y;
                bits.1[10] = core.global.cursor_projected_pos.z;
            } else if core.global.mouse_grabbed_state {
                rwin.set_cursor_visible(true);
                rwin.set_cursor_grab(CursorGrabMode::None);
                core.global.mouse_grabbed_state = false;
            }

            if core.input_manager.update(&event) {
                controls::controls_evaluate(&mut core, control_flow);
                // frame!("START");

                core.global.mouse_delta = vec2(0., 0.);
                // frame!("END");
                // frame!();
            }

            match event {
                Event::RedrawEventsCleared => {
                    core.loop_helper.loop_start(); //
                                                   // #[cfg(not(target_arch = "wasm32"))]

                    rwin.request_redraw();
                }
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
                Event::RedrawRequested(_) => {
                    state_change_check(&mut core, control_flow, &mut rwin);
                    // Run our update and look for a "loop complete" return call from the bundle manager calling the lua loop in a previous step.
                    // The lua context upon completing a loop will send a MainCommmand::LoopComplete to this thread.
                    if let Some(buff) = core.update(&mut catcher, &mut updated_bundles) {
                        instance_buffers = buff;
                    }
                    core.bundle_manager.call_loop(&mut updated_bundles, bits);

                    match core.render(&instance_buffers) {
                        Ok(_) => {}
                        // Reconfigure the surface if lost
                        // Err(wgpu::SurfaceError::Lost) => core.resize(core.size),
                        // The system is out of memory, we should probably quit
                        // Err(wgpu::SurfaceError::OutOfMemory) => *control_flow = ControlFlow::Exit,
                        // All other errors (Outdated, Timeout) should be resolved by the next frame
                        // Err(e) => eprintln!("{:?}", e),
                        _ => {}
                    };

                    core.loop_helper.loop_sleep();
                }
                _ => {}
            }
        });
    }
    #[cfg(not(feature = "headed"))]
    {
        // loop
        let mut updated_bundles = FxHashMap::default();
        loop {
            core.loop_helper.loop_start();
            if let Ok(inp) = core.cli_thread_receiver.try_recv() {
                core_console_command(&mut core, &inp);
            }
            if state_change_check(&mut core, &mut (), &mut ()) {
                return;
            }
            core.update(&mut updated_bundles);
            core.bundle_manager.call_loop(&mut updated_bundles, bits);
            core.loop_helper.loop_sleep();
        }
    }
}

pub fn core_console_command(core: &mut Core, com_in: &str) {
    let mut com = com_in.trim().to_owned();
    if let Some(alias) = core.global.aliases.get(&com) {
        com = alias.to_string();
    }
    for c in com.split("&&") {
        match crate::command::run_con_sys(core, c) {
            Ok(false) => {
                let mut ltype = LogType::Lua;
                // TODO this should use the async sender, otherwise it will block the main thread if lua is lagging
                if let Some(result) = match core.bundle_manager.get_lua().func(c) {
                    LuaResponse::String(s) => Some(s),
                    LuaResponse::Number(n) => Some(n.to_string()),
                    LuaResponse::Integer(i) => Some(i.to_string()),
                    LuaResponse::Boolean(b) => Some(b.to_string()),
                    LuaResponse::Table(t) => {
                        let mut s = String::new();
                        s.push_str("{");
                        for (k, v) in t {
                            s.push_str(&format!("{}: {}, ", k, v));
                        }
                        s.push_str("}");
                        Some(s)
                    }
                    LuaResponse::Error(e) => {
                        ltype = LogType::LuaError;
                        Some(e)
                    } // LuaResponse::Function(f) => format!("function: {}", f),

                    _ => Some("~".to_string()), // ignore nils
                } {
                    core.loggy.log(ltype, &result);
                }
            }
            Ok(true) => {}
            Err(e) => {
                core.loggy.log(LogType::LuaError, &format!("!!{}", e));
            }
        }
    }
}

#[cfg(feature = "headed")]
type IB = InstanceBuffer;
#[cfg(not(feature = "headed"))]
type IB = ();

impl Core {
    fn update(
        &mut self,
        catcher: &mut Receiver<MainPacket>,
        completed_bundles: &mut FxHashMap<u8, bool>,
    ) -> Option<IB> {
        let mut loop_complete = false;
        let mut only_one_gui_sync = true;
        catcher.try_iter().for_each(|(id, p)| {
            match p {
                MainCommmand::Cam(p, r) => {
                    if let Some(pos) = p {
                        self.global.cam_pos = pos;
                    }
                    if let Some(rot) = r {
                        self.global.simple_cam_rot = rot;
                    }
                }
                MainCommmand::GetImg(s, tx) => {
                    #[cfg(feature = "headed")]
                    self.log_check(tx.send(self.tex_manager.get_img(&s)));
                }
                MainCommmand::SetImg(s, im, tx) => {
                    #[cfg(feature = "headed")]
                    {
                        self.tex_manager.overwrite_texture(
                            &s,
                            im,
                            &mut self.world,
                            id,
                            &mut self.loggy,
                        );
                        self.log_check(tx.send(()));
                        self.tex_manager
                            .refinalize(&self.gfx.queue, &self.gfx.master_texture);
                    }
                }
                MainCommmand::Anim(name, items, speed) => {
                    #[cfg(feature = "headed")]
                    {
                        let frames = items
                            .iter()
                            .map(|i| self.tex_manager.get_tex(i))
                            .collect_vec();
                        if frames.len() == 0 {
                            self.loggy.log(
                                log::LogType::TextureError,
                                &format!("Animation {} has no frames, not storing", name),
                            );
                        } else {
                            self.tex_manager.animations.insert(
                                name,
                                crate::texture::Anim {
                                    frames,
                                    speed,
                                    once: false,
                                },
                            );
                        }
                    }
                }
                MainCommmand::Model(model) => {
                    let res = self.model_manager.upsert_model(
                        #[cfg(feature = "headed")]
                        &self.gfx.device,
                        #[cfg(feature = "headed")]
                        &self.tex_manager,
                        &mut self.world,
                        id,
                        &model.asset,
                        model.textures,
                        model.vecs,
                        model.norms,
                        model.inds,
                        model.uvs,
                        model.style,
                        &mut self.loggy,
                        self.global.debug,
                    );
                    if let Some(m) = res {
                        self.global.state_changes.push(StateChange::ModelChange(m));
                        self.global.is_state_changed = true;
                    }

                    self.log_check(model.sender.send(0));
                }
                MainCommmand::ListModel(s, bundles, tx) => {
                    let list = self.model_manager.search_model(&s, bundles);
                    self.log_check(tx.send(list));
                }
                MainCommmand::Make(m, tx) => {
                    if m.len() == 7 {
                        // change order to match expectations from the front end
                        let m2 = vec![
                            m[1].clone(),
                            m[6].clone(),
                            m[2].clone(),
                            m[4].clone(),
                            m[3].clone(),
                            m[5].clone(),
                        ];
                        self.model_manager.edit_cube(
                            #[cfg(feature = "headed")]
                            &self.gfx.device,
                            #[cfg(feature = "headed")]
                            &self.tex_manager,
                            &mut self.world,
                            id,
                            m[0].clone(),
                            m2,
                        );

                        self.log_check(tx.send(0));
                    }
                }
                MainCommmand::Spawn(lent) => {
                    self.ent_manager.create_from_lua(
                        #[cfg(feature = "headed")]
                        &self.tex_manager,
                        #[cfg(feature = "headed")]
                        &self.model_manager,
                        lent,
                    );
                }
                MainCommmand::Group(parent, child, tx) => {
                    self.ent_manager.group(parent, child);
                    self.log_check(tx.send(true));
                }
                MainCommmand::Globals(table) => {
                    for (k, v) in table.iter() {
                        #[cfg(feature = "headed")]
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

                                self.gfx.set_window_size(arr.get(0), arr.get(1));
                            }
                            "title" => {
                                if let Some(s) = Self::val2string(v) {
                                    self.gfx.set_title(&s);
                                }
                            }
                            "lock" => {
                                self.global.console = false;
                                self.gui.disable_console();
                                self.global.locked = true;
                            }

                            _ => {}
                        }
                        #[cfg(not(feature = "headed"))]
                        match k.as_str() {
                            "lock" => {
                                self.global.console = false;
                                self.global.locked = true;
                            }
                            _ => {}
                        }
                    }
                }
                MainCommmand::GetGlobal(tx) => {
                    let t = GlobalMap::new(OS, 60., self.global.gui_params.resolution);
                    self.log_check(tx.send(t));
                }
                MainCommmand::AsyncError(e) => {
                    let s = format!("!!{}", e);
                    self.log(log::LogType::LuaError, &s);
                }
                MainCommmand::Read(path, tx) => {
                    println!("read {}", path);
                    let pak = match self.bundle_manager.get_main_bundle().get_directory() {
                        Some(dir) => match crate::file_util::get_file_string_scrubbed(dir, &path) {
                            Ok(s) => Some(s),
                            Err(e) => {
                                self.log(LogType::IoError, &format!("!!{}", e));
                                None
                            }
                        },
                        None => {
                            self.log(LogType::IoError, &format!("!! No relative path access"));
                            None
                        }
                    };
                    let res = tx.send(pak);
                    self.log_check(res);
                }
                MainCommmand::Write(file, contents, tx) => {
                    let res = match self.bundle_manager.get_main_bundle().get_directory() {
                        Some(dir) => {
                            if let Err(e) =
                                crate::file_util::write_file_string_scrubbed(dir, &file, &contents)
                            {
                                self.log(LogType::IoError, &format!("!!{}", e));
                                false
                            } else {
                                true
                            }
                        }
                        None => {
                            self.log(LogType::IoError, &format!("!! No relative path access"));
                            false
                        }
                    };
                    self.log_check(tx.send(res));
                }
                MainCommmand::BundleDropped(b) => {
                    completed_bundles.remove(&id);
                    self.bundle_manager.reclaim_resources(b);
                }
                MainCommmand::Subload(file, is_overlay) => {
                    crate::command::load(
                        self,
                        Some(file.as_str()),
                        None,
                        None,
                        Some((id, is_overlay)),
                    );
                }
                MainCommmand::Reload() => crate::command::reload(self, id),

                MainCommmand::WorldSync(chunks, dropped) => {
                    self.world.process_sync(
                        #[cfg(feature = "headed")]
                        &self.gfx.device,
                        id,
                        chunks,
                        dropped,
                        &self.model_manager,
                    );
                }
                MainCommmand::Stats() => {
                    self.world.stats();
                }
                MainCommmand::Quit(u) => {
                    if u > 0 {
                        // println!(
                        //     "quit with pending load {} {:?}",
                        //     u, self.global.pending_load
                        // );
                        self.global.pending_load = None;
                        match &self.global.pending_load {
                            Some(l) => {
                                let to_load = l.clone();
                                self.log(LogType::Sys, &format!("load {}", l));
                                crate::command::hard_reset(self);

                                crate::command::load(self, Some(&to_load), None, None, None);
                            }
                            _ => {
                                //DEV if a load quit is triggered and then the lua context spams it too fast it technically quits to empty comnsole. should ahve it be code based or not trigger too quickly
                                crate::command::hard_reset(self);
                                crate::command::load_empty(self);
                            }
                        }
                    } else {
                        self.global.state_changes.push(StateChange::Quit);
                    }
                    self.global.is_state_changed = true;
                }
                MainCommmand::LoopComplete(img_result) => {
                    if let Some(main) = img_result.0 {
                        if !self.bundle_manager.is_single() {
                            self.bundle_manager.set_raster(id, 0, main);
                            #[cfg(feature = "headed")]
                            if only_one_gui_sync {
                                // DEV is this still necessary to force 1? Wont that break simultaneous syncs?
                                self.update_raster(ScreenIndex::Primary);
                                only_one_gui_sync = false;
                            }
                        } else {
                            #[cfg(feature = "headed")]
                            self.gui.replace_image(main, ScreenIndex::Primary);
                        }
                    }
                    if let Some(sky) = img_result.1 {
                        if !self.bundle_manager.is_single() {
                            self.bundle_manager.set_raster(id, 1, sky);
                            #[cfg(feature = "headed")]
                            if only_one_gui_sync {
                                self.update_raster(ScreenIndex::Sky);
                                only_one_gui_sync = false;
                            }
                        } else {
                            #[cfg(feature = "headed")]
                            self.gui.replace_image(sky, gui::ScreenIndex::Sky);
                        }
                    }
                    completed_bundles.insert(id, true);
                    loop_complete = true;
                }
                MainCommmand::Copy(s) => {
                    if let Ok(mut ctx) = ClipboardContext::new() {
                        if let Err(_) = ctx.set_contents(s) {
                            self.log(LogType::IoError, &format!("!!Clipboard error"));
                        }
                    }
                }
                MainCommmand::Load(_) => todo!(),
                MainCommmand::Null() => todo!(),
            }
        });

        let instance_buffers = if loop_complete {
            Some(self.ent_manager.check_ents(
                #[cfg(feature = "headed")]
                &self.gfx.device,
                #[cfg(feature = "headed")]
                &self.tex_manager,
                #[cfg(feature = "headed")]
                &self.model_manager,
                self.global.iteration,
            ))
        } else {
            None
        };

        self.global.iteration += 1;
        instance_buffers
    }
}

pub fn error_window(e: Box<dyn std::error::Error>) {
    // #[cfg(target_os = "windows")]
    // {
    //     use std::ptr::null_mut as NULL;
    //     use winapi::um::winuser;
    //     let st = format!("{}\0", e.to_string());
    //     let l_msg: Vec<u16> = st.encode_utf16().collect();
    //     // let l_msg: Vec<u16> = "Wassa wassa wassup\0".encode_utf16().collect();
    //     let l_title: Vec<u16> = "Petrichor64 Error\0".encode_utf16().collect();

    //     unsafe {
    //         winuser::MessageBoxW(
    //             NULL(),
    //             l_msg.as_ptr(),
    //             l_title.as_ptr(),
    //             winuser::MB_OK | winuser::MB_ICONINFORMATION,
    //         );
    //     }
    // }
    native_dialog::MessageDialog::new()
        .set_type(native_dialog::MessageType::Error)
        .set_title("Petrichor64 Error")
        // .set_text(&format!("{:#?}", path))
        .set_text(&e.to_string())
        .show_alert()
        .unwrap();
}

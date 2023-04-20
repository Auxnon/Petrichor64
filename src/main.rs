use std::rc::Rc;

#[cfg(feature = "headed")]
use ent_manager::InstanceBuffer;
use glam::vec2;
use global::StateChange;
use gui::ScreenIndex;
use itertools::Itertools;
#[cfg(feature = "headed")]
use root::Core;
#[cfg(not(feature = "headed"))]
use root_headless::Core;
use rustc_hash::FxHashMap;
use types::ControlState;

mod asset;
mod bundle;
mod command;
#[cfg(feature = "headed")]
mod controls;
#[cfg(feature = "headed")]
mod ent;
mod ent_manager;
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
mod zip_pal;

use command::MainCommmand;
#[cfg(feature = "headed")]
use winit::{
    dpi::LogicalSize,
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{self, ControlFlow, EventLoop},
    window::{CursorGrabMode, WindowBuilder},
};

fn main() {
    crate::parse::test(&"test.lua".to_string());
    env_logger::init();

    #[cfg(feature = "headed")]
    let (mut core, mut rwin, center, event_loop) = {
        let event_loop = EventLoop::new();
        let win = WindowBuilder::new()
            .with_inner_size(winit::dpi::LogicalSize::new(640i32, 480i32))
            .build(&event_loop)
            .unwrap();
        win.set_title("Petrichor64");

        let center = winit::dpi::LogicalPosition::new(320.0f64, 240.0f64);
        let rwindow = Rc::new(win);

        // State::new uses async code, so we're going to wait for it to finish
        (
            pollster::block_on(Core::new(Rc::clone(&rwindow))),
            rwindow,
            center,
            event_loop,
        )
    };

    #[cfg(not(feature = "headed"))]
    let mut core = pollster::block_on(Core::new());

    crate::command::load_empty(&mut core);

    core.loggy.clear();

    core.global.state_changes.push(StateChange::Config);
    // DEV a little delay trick to ensure any pending requests in our "console" app are completed before the following state change is made
    core.global.state_delay = 8;
    core.global.is_state_changed = true;
    let mut bits: ControlState = ([false; 256], [0.; 11]);

    match crate::asset::check_for_auto() {
        Some(s) => {
            core.global.console = false;
            #[cfg(feature = "headed")]
            core.gui.disable_console();
            // crate::command::hard_reset(&mut core);

            core.global.pending_load = Some(s.clone());
            // crate::command::load_from_string(&mut core, Some(s));
            core.bundle_manager.get_lua().call_drop(s);
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

    #[cfg(not(feature = "headed"))]
    type Param1 = ();
    #[cfg(not(feature = "headed"))]
    type Param2 = ();
    #[cfg(feature = "headed")]
    type Param1 = ControlFlow;
    #[cfg(feature = "headed")]
    type Param2 = Rc<winit::window::Window>;

    let mut state_change_check =
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
                                    println!("hi config: {:?}", s);
                                    crate::command::init_con_sys(c, &s);
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
                    if let Some(buff) = core.update(&mut updated_bundles) {
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
            if state_change_check(&mut core, &mut (), &mut ()) {
                return;
            }
            core.update(&mut updated_bundles);
            core.bundle_manager.call_loop(&mut updated_bundles, bits);
        }
    }

    // unsafe {
    //     tracy::shutdown_tracy();
    // }
}

#[cfg(feature = "headed")]
type IB = InstanceBuffer;
#[cfg(not(feature = "headed"))]
type IB = ();

impl Core {
    fn update(&mut self, completed_bundles: &mut FxHashMap<u8, bool>) -> Option<IB> {
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

                MainCommmand::GetImg(s, tx) => {
                    #[cfg(feature = "headed")]
                    tx.send(self.tex_manager.get_img(&s));
                }
                MainCommmand::SetImg(s, im) => {
                    #[cfg(feature = "headed")]
                    {
                        self.tex_manager.overwrite_texture(
                            &s,
                            im,
                            &mut self.world,
                            id,
                            &mut self.loggy,
                        );
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
                MainCommmand::Model(name, texture, v, n, i, u, style, tx) => {
                    let res = self.model_manager.upsert_model(
                        #[cfg(feature = "headed")]
                        &self.gfx.device,
                        #[cfg(feature = "headed")]
                        &self.tex_manager,
                        &mut self.world,
                        id,
                        &name,
                        texture,
                        v,
                        n,
                        i,
                        u,
                        style,
                        &mut self.loggy,
                        self.global.debug,
                    );
                    if let Some(m) = res {
                        self.global.state_changes.push(StateChange::ModelChange(m));
                        self.global.is_state_changed = true;
                    }

                    tx.send(0);
                }
                MainCommmand::ListModel(s, bundles, tx) => {
                    let list = self.model_manager.search_model(&s, bundles);
                    tx.send(list);
                }
                MainCommmand::Make(m, tx) => {
                    if m.len() == 7 {
                        // change order to match expecations from the front end
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

                        tx.send(0);
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
                    tx.send(true);
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
                MainCommmand::AsyncError(e) => {
                    let s = format!("!!{}", e);
                    #[cfg(feature = "headed")]
                    self.gui.push_notif(&s);
                    self.loggy.log(log::LogType::LuaError, &s);
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
                        println!(
                            "quit with pending load {} {:?}",
                            u, self.global.pending_load
                        );
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
                    if let Some(main) = img_result.0 {
                        if !self.bundle_manager.is_single() {
                            self.bundle_manager.set_raster(id, 0, main);
                            #[cfg(feature = "headed")]
                            mutations.push((id, MainCommmand::Meta(ScreenIndex::Primary)));
                        } else {
                            #[cfg(feature = "headed")]
                            self.gui.replace_image(main, ScreenIndex::Primary);
                        }
                    }
                    if let Some(sky) = img_result.1 {
                        if !self.bundle_manager.is_single() {
                            self.bundle_manager.set_raster(id, 1, sky);
                            #[cfg(feature = "headed")]
                            mutations.push((id, MainCommmand::Meta(ScreenIndex::Sky)));
                        } else {
                            #[cfg(feature = "headed")]
                            self.gui.replace_image(sky, gui::ScreenIndex::Sky);
                        }
                    }
                    completed_bundles.insert(id, true);
                    loop_complete = true;
                }
                MainCommmand::Load(_) => todo!(),
                MainCommmand::Null() => todo!(),
                MainCommmand::Meta(_) => todo!(),
            };
        }

        if !mutations.is_empty() {
            let mut only_one_gui_sync = true;
            for (id, m) in mutations {
                match m {
                    MainCommmand::Reload() => crate::command::reload(self, id),
                    MainCommmand::Quit(u) => {
                        //DEV if a load quit is triggered and then the lua context spams it too fast it technically quits to empty comnsole. should ahve it be code based or not trigger too quickly
                        // crate::command::hard_reset(self);
                        // crate::command::load_empty(self);
                    }
                    MainCommmand::Subload(file, is_overlay) => {
                        crate::command::load(self, Some(file), None, None, Some((id, is_overlay)));
                    }
                    MainCommmand::Load(file) => {
                        crate::command::hard_reset(self);
                        println!("load {}", file);
                        crate::command::load(self, Some(file), None, None, None);
                    }
                    #[cfg(feature = "headed")]
                    MainCommmand::Meta(d) => {
                        if only_one_gui_sync {
                            match self.bundle_manager.get_rasters(match d {
                                ScreenIndex::Primary => 0,
                                _ => 1,
                            }) {
                                Some(rasters) => {
                                    self.gui.replace_image(rasters, d);
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

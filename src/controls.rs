use std::sync::Arc;

use glam::{IVec2, IVec3};
use once_cell::sync::OnceCell;
use rand::Error;
use winit::event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;

use crate::lua_define::LuaCore;
use crate::{ent_master, lua_master, Core};

pub fn controls_evaluate(event: &WindowEvent, core: &mut Core, control_flow: &mut ControlFlow) {
    match event {
        WindowEvent::CloseRequested
        | WindowEvent::KeyboardInput {
            input:
                KeyboardInput {
                    state: ElementState::Pressed,
                    virtual_keycode: Some(VirtualKeyCode::Escape),
                    ..
                },
            ..
        } => *control_flow = ControlFlow::Exit,
        // WindowEvent::KeyboardInput {
        //     input:
        //         KeyboardInput {
        //             state: ElementState::Pressed,
        //             virtual_keycode: Some(VirtualKeyCode::Space),
        //             ..
        //         },
        //     ..
        // } => {
        //     state.switch_board.write().space = true;
        //     //globals.write().space = true;
        // }
        // WindowEvent::KeyboardInput {
        //     input:
        //         KeyboardInput {
        //             state: ElementState::Released,
        //             virtual_keycode: Some(VirtualKeyCode::Space),
        //             ..
        //         },
        //     ..
        // } => {
        //     state.switch_board.write().space = false;
        //     let input_path = std::path::Path::new("scripts").join("walker.lua");

        //     state
        //         .gui
        //         .add_text(std::fs::read_to_string(input_path).unwrap_or_default());
        //     //globals.write().space = false;
        // }
        WindowEvent::DroppedFile(path) => {
            println!("dropped file {}", path.as_os_str().to_string_lossy());
            // winit::ControlFlow::Continue
        }
        WindowEvent::Resized(physical_size) => {
            core.resize(*physical_size);
        }
        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
            // new_inner_size is &mut so w have to dereference it twice
            core.resize(**new_inner_size);
        }
        _ => {
            let space_down = core.input_helper.key_held(VirtualKeyCode::Space);
            core.switch_board.write().space = space_down;
            if space_down {

                // Some(d) => {}
                // None => {
                //     match lua_master.try_lock() {
                //         Some(lua_guard) => {
                //             println!("yr");
                //             let lua_core = lua_guard.get_or_init();
                //             parking_lot::MutexGuard::unlock_fair(lua_guard);
                //             crate::asset::init(&state.device);
                //         }
                //         _ => {
                //             println!("naw");
                //         }
                //     }
                //     //
                // }
            }
            let diff = core.input_helper.mouse_diff();
            // core.global.mouse_delta.x = diff.0;
            // core.global.mouse_delta.y = diff.1;
            // core.global.mouse_active_pos.x += diff.0 / 10000.;
            // core.global.mouse_active_pos.y += diff.1 / 10000.;

            // match core.input_helper.mouse() {
            //     Some((x, y)) => {
            //         // core.global.mouse_active_pos.x = x / core.size.width as f32;
            //         core.global.mouse_active_pos.y = y / core.size.height as f32;
            //     }
            //     _ => {}
            // }

            if core.input_helper.mouse_held(0) {}
            if core.input_helper.mouse_pressed(0) {
                core.global.mouse_click_pos = core.global.mouse_active_pos.clone();
                // core.global.set("value2".to_string(), 1.);
                let i = (core.global.cursor_projected_pos / 1.).floor() * 16.;
                // println!("i pos {}", i);
                core.world
                    .set_tile("grid".to_string(), i.x as i32, i.y as i32, i.z as i32);
                core.world
                    .get_chunk_mut(i.x as i32, i.y as i32, i.z as i32)
                    .cook(&core.device);
            }
            if core.input_helper.key_pressed(VirtualKeyCode::Left) {
                core.global.camera_pos.x += 10.;
                println!("x {}", core.global.camera_pos.x)
            } else if core.input_helper.key_pressed(VirtualKeyCode::Right) {
                core.global.camera_pos.x -= 10.;
                println!("x {}", core.global.camera_pos.x)
            } else if core.input_helper.key_pressed(VirtualKeyCode::Up) {
                if core.input_helper.held_shift() {
                    core.global.camera_pos.z += 10.;
                    println!("z {}", core.global.camera_pos.z)
                } else {
                    core.global.camera_pos.y += 10.;
                    println!("y {}", core.global.camera_pos.y)
                }
            } else if core.input_helper.key_pressed(VirtualKeyCode::Down) {
                if core.input_helper.held_shift() {
                    core.global.camera_pos.z -= 10.;
                    println!("z {}", core.global.camera_pos.z)
                } else {
                    core.global.camera_pos.y -= 10.;
                    println!("y {}", core.global.camera_pos.y)
                }
            }

            if core.input_helper.key_released(VirtualKeyCode::Grave) {
                core.global.console = !core.global.console;
                if core.global.console {
                    core.gui.enable_output()
                } else {
                    core.gui.disable_output()
                }
            } else if core.global.console {
                if core.input_helper.key_released(VirtualKeyCode::Return) {
                    let command = crate::log::carriage();
                    if command.is_some() {
                        if core.global.test {
                            // println!("command isss {}", command.unwrap());
                            let com = command.unwrap();
                            if !sys_command(&core, &com) {
                                let guard = crate::lua_master.lock();
                                let lua_core = guard.get();
                                if lua_core.is_some() {
                                    let result = lua_core.unwrap().func(&com);
                                    crate::log::log(result.clone());

                                    crate::log::next_line();
                                    println!("command was {}, result was {}", com, result);
                                } else {
                                    crate::log::log("uwu".to_string());
                                    crate::log::next_line();
                                }
                            }
                        } else {
                            core.global.test = true;
                        }
                    }
                } else if core.input_helper.key_pressed(VirtualKeyCode::Up) {
                    crate::log::history()
                // } else if core.input_helper.key_released(VirtualKeyCode::Back) {
                // crate::log::back();
                } else {
                    let t = core.input_helper.text();

                    if t.len() > 0 {
                        let mut neg = 0;
                        let emp: char;
                        let mut st = vec![];
                        for tt in t.iter() {
                            match tt {
                                winit_input_helper::TextChar::Char(c) => match *c as u32 {
                                    127 => {
                                        crate::log::back();
                                    }
                                    _ => st.push(*c),
                                },
                                winit_input_helper::TextChar::Back => {
                                    crate::log::back();
                                    // neg += 1;
                                    // st.remove(st.len() - 1);
                                }
                            }
                        }
                        crate::log::add(String::from_iter(st.clone()));

                        // println!(" char {}", String::from_iter(st));
                    }
                }
            }
        }
    }

    // match event {
    //     WindowEvent::KeyboardInput {
    //         input:
    //             KeyboardInput {
    //                 core: ElementState::Pressed,
    //                 virtual_keycode: Some(key_out),
    //             },
    //         ..
    //     } => println!("{}", key_out),
    //     _ => {}
    // }
}

/** Private commands not reachable by lua code, but also works without lua being loaded */
pub fn sys_command(core: &Core, s: &String) -> bool {
    match s.as_str() {
        "$die" => {
            // this chunk could probably be passed directly to lua core but being it's significance it felt important to pass into our pre-system check for commands
            let guard = crate::lua_master.lock();
            let lua_core = guard.get();
            if lua_core.is_some() {
                lua_core.unwrap().die();
            }
        }
        "$pack" => {
            //
            crate::asset::pack();
        }
        "$unpack" => {
            //
            crate::zip_pal::unpack_and_save(&"biggo.png".to_string(), &"biggo.zip".to_string());
        }
        "$load" => {
            crate::texture::reset();
            crate::asset::unpack(&core.device, &"biggo.png".to_string());

            let mutex = crate::lua_master.lock();
            match mutex.get() {
                Some(d) => {}
                None => {
                    mutex.get_or_init(|| {
                        crate::lua_define::LuaCore::new(Arc::clone(&core.switch_board))
                    });
                    // crate::texture::reset();
                    // crate::asset::init(&core.device);
                    let mut mutex = crate::ent_master.lock();
                    let entity_manager = mutex.get_mut().unwrap();
                    crate::texture::refinalize(&core.device, &core.queue, &core.master_texture);
                    for e in &mut entity_manager.entities {
                        e.hot_reload();
                    }
                    log("buldozed into this here code with a buncha stuff".to_string());
                }
            }
        }
        "$print_atlas" => {
            crate::texture::save_atlas();
        }
        "$ugh" => {
            //
        }
        &_ => return false,
    }
    true
}

fn log(str: String) {
    crate::log::log(format!("controls::{}", str));
    println!("{}", str);
}

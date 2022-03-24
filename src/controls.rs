use glam::{IVec2, IVec3};
use once_cell::sync::OnceCell;
use rand::Error;
use winit::event::{DeviceEvent, ElementState, Event, KeyboardInput, VirtualKeyCode, WindowEvent};
use winit::event_loop::ControlFlow;

use crate::lua_define::LuaCore;
use crate::{lua_master, State};

pub fn controls_evaluate(event: &WindowEvent, state: &mut State, control_flow: &mut ControlFlow) {
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
            state.resize(*physical_size);
        }
        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
            // new_inner_size is &mut so w have to dereference it twice
            state.resize(**new_inner_size);
        }
        _ => {
            let space_down = state.input_helper.key_held(VirtualKeyCode::Space);
            state.switch_board.write().space = space_down;
            if space_down {
                let mutex = crate::lua_master.lock();
                // mutex.get_or_init::<LuaCore>(|| Ok(crate::lua_define::LuaCore::new()));
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
            let diff = state.input_helper.mouse_diff();
            // state.global.mouse_delta.x = diff.0;
            // state.global.mouse_delta.y = diff.1;
            // state.global.mouse_active_pos.x += diff.0 / 10000.;
            // state.global.mouse_active_pos.y += diff.1 / 10000.;

            // match state.input_helper.mouse() {
            //     Some((x, y)) => {
            //         // state.global.mouse_active_pos.x = x / state.size.width as f32;
            //         state.global.mouse_active_pos.y = y / state.size.height as f32;
            //     }
            //     _ => {}
            // }

            if state.input_helper.mouse_held(0) {}
            if state.input_helper.mouse_pressed(0) {
                state.global.mouse_click_pos = state.global.mouse_active_pos.clone();
                // state.global.set("value2".to_string(), 1.);
                let i = (state.global.cursor_projected_pos / 1.).floor() * 16.;
                // println!("i pos {}", i);
                state
                    .world
                    .set_tile("grid".to_string(), i.x as i32, i.y as i32, i.z as i32);
                state
                    .world
                    .get_chunk_mut(i.x as i32, i.y as i32, i.z as i32)
                    .cook(&state.device);
            }
            if state.input_helper.key_pressed(VirtualKeyCode::Left) {
                state.global.camera_pos.x += 10.;
                println!("x {}", state.global.camera_pos.x)
            } else if state.input_helper.key_pressed(VirtualKeyCode::Right) {
                state.global.camera_pos.x -= 10.;
                println!("x {}", state.global.camera_pos.x)
            } else if state.input_helper.key_pressed(VirtualKeyCode::Up) {
                if state.input_helper.held_shift() {
                    state.global.camera_pos.z += 10.;
                    println!("z {}", state.global.camera_pos.z)
                } else {
                    state.global.camera_pos.y += 10.;
                    println!("y {}", state.global.camera_pos.y)
                }
            } else if state.input_helper.key_pressed(VirtualKeyCode::Down) {
                if state.input_helper.held_shift() {
                    state.global.camera_pos.z -= 10.;
                    println!("z {}", state.global.camera_pos.z)
                } else {
                    state.global.camera_pos.y -= 10.;
                    println!("y {}", state.global.camera_pos.y)
                }
            }

            if state.input_helper.key_released(VirtualKeyCode::Grave) {
                state.global.console = !state.global.console;
                if state.global.console {
                    state.gui.enable_output()
                } else {
                    state.gui.disable_output()
                }
            } else if state.global.console {
                if state.input_helper.key_released(VirtualKeyCode::Return) {
                    let command = crate::log::carriage();
                    if command.is_some() {
                        if state.global.test {
                            // println!("command isss {}", command.unwrap());
                            let guard = crate::lua_master.lock();
                            let core = guard.get();
                            if core.is_some() {
                                let com = command.unwrap();
                                let result = core.unwrap().func(com.to_owned());
                                crate::log::log(result.clone());

                                crate::log::next_line();
                                println!("command was {}, result was {}", com, result);
                            }
                        } else {
                            state.global.test = true;
                        }
                    }
                } else if state.input_helper.key_pressed(VirtualKeyCode::Up) {
                    crate::log::history()
                // } else if state.input_helper.key_released(VirtualKeyCode::Back) {
                // crate::log::back();
                } else {
                    let t = state.input_helper.text();

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
                                    // crate::log::back();
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
    //                 state: ElementState::Pressed,
    //                 virtual_keycode: Some(key_out),
    //             },
    //         ..
    //     } => println!("{}", key_out),
    //     _ => {}
    // }
}

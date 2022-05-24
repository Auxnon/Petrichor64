use clipboard::{ClipboardContext, ClipboardProvider};
use winit::{
    event::{ElementState, KeyboardInput, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

use crate::Core;

#[cfg(target_os = "macos")]
const COMMAND_KEY_L: VirtualKeyCode = VirtualKeyCode::LWin;
#[cfg(not(target_os = "macos"))]
const COMMAND_KEY_L: VirtualKeyCode = VirtualKeyCode::LControl;

#[cfg(target_os = "macos")]
const COMMAND_KEY_R: VirtualKeyCode = VirtualKeyCode::RWin;
#[cfg(not(target_os = "macos"))]
const COMMAND_KEY_R: VirtualKeyCode = VirtualKeyCode::RControl;

pub fn controls_evaluate(core: &mut Core, control_flow: &mut ControlFlow) {
    // WindowEvent::Resized(physical_size) => {
    //     core.resize(*physical_size);
    // }
    // WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
    //     // new_inner_size is &mut so w have to dereference it twice
    //     core.resize(**new_inner_size);
    // }
    // _ => {
    if core.input_helper.key_pressed(VirtualKeyCode::Escape) || core.input_helper.quit() {
        *control_flow = ControlFlow::Exit;
    }
    match core.input_helper.dropped_file() {
        Some(path) => println!("dropped file {}", path.as_os_str().to_string_lossy()),
        _ => {}
    }
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
            crate::log::add("$load".to_string());
            core.gui.enable_output()
        } else {
            core.gui.disable_output()
        }
    } else if core.global.console {
        if core.input_helper.key_released(VirtualKeyCode::Return) {
            let command = crate::log::carriage();
            if command.is_some() {
                // if core.global.test {
                let com = command.unwrap();
                println!("command isss {}", com);

                if !crate::command::init_con_sys(&core, &com) {
                    let guard = crate::lua_master.lock();
                    let lua_core = guard.get();
                    if lua_core.is_some() {
                        let result = lua_core.unwrap().func(&com);
                        crate::log::log(result.clone());

                        // crate::log::next_line();
                        println!("command was {}, result was {}", com, result);
                    } else {
                        crate::log::log("uwu".to_string());
                        // crate::log::next_line();
                    }
                }
                // } else {
                //     core.global.test = true;
                // }
            }
        } else if core.input_helper.key_pressed(VirtualKeyCode::Up) {
            crate::log::history_up()
        } else if core.input_helper.key_pressed(VirtualKeyCode::Down) {
            crate::log::history_down()
        } else if core.input_helper.key_held(COMMAND_KEY_L)
            || core.input_helper.key_held(COMMAND_KEY_R)
        {
            if core.input_helper.key_pressed(VirtualKeyCode::C) {
                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                ctx.set_contents(crate::log::get_line()).unwrap();
            } else if core.input_helper.key_pressed(VirtualKeyCode::V) {
                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                match ctx.get_contents() {
                    Ok(s) => crate::log::add(s),
                    _ => {}
                }
            }
        } else {
            let t = core.input_helper.text();
            // core.input_helper.

            if t.len() > 0 {
                let neg = 0;
                let emp: char;

                // let mut st = vec![];
                match t.last() {
                    Some(s) => {
                        match s {
                            winit_input_helper::TextChar::Char(c) => match *c as u32 {
                                127 => {
                                    crate::log::back();
                                }
                                _ => {
                                    println!("char {}  {}", t.len(), c);
                                    crate::log::add(String::from(*c))
                                } //st.push(*c),
                            },
                            winit_input_helper::TextChar::Back => {
                                // crate::log::back();
                                // neg += 1;
                                // st.remove(st.len() - 1);
                            }
                        }
                    }
                    None => {}
                }

                // for tt in t.iter() {
                //     match tt {
                //         // DEV just 127 is fine on mac, windows may have need 127 and :Back in order to work
                //         winit_input_helper::TextChar::Char(c) => match *c as u32 {
                //             127 => {
                //                 crate::log::back();
                //             }
                //             _ => st.push(*c),
                //         },
                //         winit_input_helper::TextChar::Back => {
                //             // crate::log::back();
                //             // neg += 1;
                //             // st.remove(st.len() - 1);
                //         }
                //     }
                // }

                // crate::log::add(String::from_iter(st.clone()));

                // println!(" char {}", String::from_iter(st));
            }
        }
    }
}

fn log(str: String) {
    crate::log::log(format!("controls::{}", str));
    println!("{}", str);
}

use crate::log::LogType;
use crate::lua_define::LuaResponse;
use crate::{bundle::BundleManager, Core};
use clipboard::{ClipboardContext, ClipboardProvider};

use winit::{
    event::{Event, VirtualKeyCode, WindowEvent},
    event_loop::ControlFlow,
};

#[cfg(target_os = "macos")]
const COMMAND_KEY_L: VirtualKeyCode = VirtualKeyCode::LWin;
#[cfg(not(target_os = "macos"))]
const COMMAND_KEY_L: VirtualKeyCode = VirtualKeyCode::LControl;

#[cfg(target_os = "macos")]
const COMMAND_KEY_R: VirtualKeyCode = VirtualKeyCode::RWin;
#[cfg(not(target_os = "macos"))]
const COMMAND_KEY_R: VirtualKeyCode = VirtualKeyCode::RControl;

pub type ControlState = ([bool; 256], [f32; 10]);

pub fn controls_evaluate(core: &mut Core, control_flow: &mut ControlFlow) {
    // WindowEvent::Resized(physical_size) => {
    //     core.resize(*physical_size);
    // }
    // WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
    //     // new_inner_size is &mut so w have to dereference it twice
    //     core.resize(**new_inner_size);
    // }
    // _ => {

    // for m in core.lua_master.catcher.try_recv() {
    //     let (ind, val, a, b, c, channel) = m;
    //     match ind {
    //         0 => {

    //         1 => {
    //             channel.send(if core.world.is_tile(a, b, c) { 1 } else { 0 });
    //         }
    //         _ => {}
    //     }
    // }
    let input_helper = &core.input_manager;

    if input_helper.quit() {
        *control_flow = ControlFlow::Exit;
    }
    match input_helper.dropped_file() {
        // TODO drag and drop
        Some(path) => println!("dropped file {}", path.as_os_str().to_string_lossy()),
        _ => {}
    }

    // TODO mouse inputs
    match input_helper.mouse() {
        Some((x, y)) => {
            core.global.game_controller = false;
            core.global.mouse_pos.x = x / core.size.width as f32;
            core.global.mouse_pos.y = y / core.size.height as f32;
            // TODO test if this is not needed on WinOS
            // let diff = input_helper.mouse_diff();
            // core.global.mouse_delta.x = diff.0;
            // core.global.mouse_delta.y = diff.1;

            // println!(core.global.)
            // input_helper.
        }
        _ => {}
    }
    // input_helper.mouse
    core.global.scroll_delta = input_helper.scroll_diff();

    // core.input_helper.key_pressed(check_key_code)
    core.global.mouse_buttons = [
        if core.input_manager.mouse_held(0) {
            1.
        } else {
            0.
        },
        if core.input_manager.mouse_held(1) {
            1.
        } else {
            0.
        },
        if core.input_manager.mouse_held(2) {
            1.
        } else {
            0.
        },
        if core.input_manager.mouse_held(3) {
            1.
        } else {
            0.
        },
    ];

    if input_helper.key_released(VirtualKeyCode::Grave) {
        if !core.global.locked {
            core.global.console = !core.global.console;
            if core.global.console {
                core.gui.enable_console(&core.loggy)
            } else {
                core.gui.disable_console()
            }
        }
    } else if core.global.console {
        if core.global.scroll_delta != 0. {
            core.loggy.scroll(core.global.scroll_delta);
        }
        if input_helper.key_released(VirtualKeyCode::Return) {
            let command = core.loggy.carriage();
            if command.is_some() {
                // if core.global.test {
                let mut com = command.unwrap(); //.to_lowercase();
                println!("command is {}", com);
                if let Some(alias) = core.global.aliases.get(&com) {
                    com = alias.to_string();
                }
                for c in com.split("&&") {
                    if !crate::command::init_con_sys(core, c) {
                        let mut ltype = LogType::Lua;
                        // TODO this should use the async sender, otherwise it will block the main thread if lua is lagging
                        let result = match core.bundle_manager.get_lua().func(c) {
                            LuaResponse::String(s) => s,
                            LuaResponse::Number(n) => n.to_string(),
                            LuaResponse::Integer(i) => i.to_string(),
                            LuaResponse::Boolean(b) => b.to_string(),
                            LuaResponse::Nil => "nil".to_string(),
                            LuaResponse::Table(t) => {
                                let mut s = String::new();
                                s.push_str("{");
                                for (k, v) in t {
                                    s.push_str(&format!("{}: {}, ", k, v));
                                }
                                s.push_str("}");
                                s
                            }
                            LuaResponse::Error(e) => {
                                ltype = LogType::LuaError;
                                e
                            } // LuaResponse::Function(f) => format!("function: {}", f),
                        };

                        core.loggy.log(ltype, &result);
                    }
                }
            }
        } else if input_helper.key_pressed(VirtualKeyCode::Up) {
            core.loggy.history_up()
        } else if input_helper.key_pressed(VirtualKeyCode::Down) {
            core.loggy.history_down()
        } else if input_helper.key_held(COMMAND_KEY_L) || input_helper.key_held(COMMAND_KEY_R) {
            if input_helper.key_pressed(VirtualKeyCode::C) {
                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                ctx.set_contents(core.loggy.get_line()).unwrap();
            } else if input_helper.key_pressed(VirtualKeyCode::V) {
                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                match ctx.get_contents() {
                    Ok(s) => core.loggy.add(s),
                    _ => {}
                }
            } else if input_helper.key_pressed(VirtualKeyCode::R) {
                crate::command::reload(core, core.bundle_manager.console_bundle_target);
            } else if input_helper.key_pressed(VirtualKeyCode::Escape) {
                *control_flow = ControlFlow::Exit;
            } else if input_helper.key_pressed(VirtualKeyCode::Return) {
                core.toggle_fullscreen();
                core.global.fullscreen_state = core.global.fullscreen;
            }
        } else {
            let t = input_helper.text();
            if t.len() > 0 {
                // let neg = 0;
                // let emp: char;

                t.iter().for_each(|s| {
                    match s {
                        winit_input_helper::TextChar::Char(c) => match *c as u32 {
                            96 => {}
                            127 => {
                                core.loggy.back();
                            }
                            _ => {
                                println!("char {}  {}", *c as u32, c);
                                core.loggy.add(String::from(*c))
                            } //st.push(*c),
                        },
                        winit_input_helper::TextChar::Back => {
                            // crate::log::back();
                            // neg += 1;
                            // st.remove(st.len() - 1);
                        }
                    }
                });

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
    } else {
        if core.global.debug {
            if input_helper.key_pressed(VirtualKeyCode::Left) {
                core.global.debug_camera_pos.x += 10.;
                core.loggy.log(
                    LogType::Debug,
                    &format!("x {}", core.global.debug_camera_pos.x),
                )
            } else if input_helper.key_pressed(VirtualKeyCode::Right) {
                core.global.debug_camera_pos.x -= 10.;
            } else if input_helper.key_pressed(VirtualKeyCode::Up) {
                if input_helper.held_shift() {
                    core.global.debug_camera_pos.z += 10.;
                    core.loggy.log(
                        LogType::Debug,
                        &format!("z {}", core.global.debug_camera_pos.z),
                    )
                } else {
                    core.global.debug_camera_pos.y += 10.;
                    core.loggy.log(
                        LogType::Debug,
                        &format!("y {}", core.global.debug_camera_pos.y),
                    )
                }
            } else if input_helper.key_pressed(VirtualKeyCode::Down) {
                if input_helper.held_shift() {
                    core.global.debug_camera_pos.z -= 10.;
                    core.loggy.log(
                        LogType::Debug,
                        &format!("z {}", core.global.debug_camera_pos.z),
                    )
                } else {
                    core.global.debug_camera_pos.y -= 10.;
                    core.loggy.log(
                        LogType::Debug,
                        &format!("y {}", core.global.debug_camera_pos.y),
                    )
                }
            }
        }

        if input_helper.key_held(COMMAND_KEY_L) || input_helper.key_held(COMMAND_KEY_R) {
            if input_helper.key_pressed(VirtualKeyCode::R) {
                crate::command::reload(core, core.bundle_manager.console_bundle_target);
            } else if input_helper.key_pressed(VirtualKeyCode::Return) {
                core.global.fullscreen = !core.global.fullscreen;
                core.check_fullscreen();
                println!("fullscreen {}", core.global.fullscreen);
            }
        }
    }
}

pub fn bit_check<T>(events: &winit::event::Event<T>, bits: &mut ControlState) {
    // match events{
    // winit::event::WindowEvent::KeyboardInput { device_id: (), input: (), is_synthetic: () },
    // _=>{}
    // }

    match events {
        Event::WindowEvent {
            // Note this deeply nested pattern match
            event:
                WindowEvent::KeyboardInput {
                    input:
                        winit::event::KeyboardInput {
                            // Which serves to filter out only events we actually want
                            virtual_keycode: Some(keycode),
                            state,
                            ..
                        },
                    ..
                },
            ..
        } => {
            // It also binds these handy variable names!
            match state {
                winit::event::ElementState::Pressed => {
                    // VirtualKeycode is an enum with a defined representation
                    // DEV
                    // println!("newkey is {}", *keycode as u32);
                    bits.0[*keycode as usize] = true;
                }
                winit::event::ElementState::Released => {
                    bits.0[*keycode as usize] = false;
                }
            }
        }
        _ => {}
    }
    // drop(bits);
}

// fn key_match(key: String) -> Option<VirtualKeyCode> {
//     Some(match key.to_lowercase().as_str() {
//         "a" => VirtualKeyCode::A,
//         "b" => VirtualKeyCode::B,
//         "c" => VirtualKeyCode::C,
//         "d" => VirtualKeyCode::D,
//         "e" => VirtualKeyCode::E,
//         "f" => VirtualKeyCode::F,
//         "g" => VirtualKeyCode::G,
//         "h" => VirtualKeyCode::H,
//         "i" => VirtualKeyCode::I,
//         "j" => VirtualKeyCode::J,
//         "k" => VirtualKeyCode::K,
//         "l" => VirtualKeyCode::L,
//         "m" => VirtualKeyCode::M,
//         "n" => VirtualKeyCode::N,
//         "o" => VirtualKeyCode::O,
//         "p" => VirtualKeyCode::P,
//         "q" => VirtualKeyCode::Q,
//         "r" => VirtualKeyCode::R,
//         "s" => VirtualKeyCode::S,
//         "t" => VirtualKeyCode::T,
//         "u" => VirtualKeyCode::U,
//         "v" => VirtualKeyCode::V,
//         "w" => VirtualKeyCode::W,
//         "x" => VirtualKeyCode::X,
//         "y" => VirtualKeyCode::Y,
//         "z" => VirtualKeyCode::Z,
//         "space" => VirtualKeyCode::Space,
//         "lctrl" => VirtualKeyCode::LControl,
//         "rctrl" => VirtualKeyCode::RControl,
//         _ => return None,
//     })
// }

fn bundle_missing(bm: &BundleManager) -> String {
    format!("please switch to target {} instead", bm.list_bundles())
}

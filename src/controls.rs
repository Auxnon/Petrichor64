use crate::lg;
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

pub type ControlState = ([bool; 256], [f32; 8]);

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
    if core.global.scroll_delta != 0. {
        crate::log::scroll(core.global.scroll_delta);
    }

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
    // println!("{:?}", core.global.mouse_buttons);
    // if input_helper.mouse_pressed(0) {
    //     core.global.mouse_click_pos = core.global.mouse_active_pos.clone();
    //     let i = (core.global.cursor_projected_pos / 1.).floor() * 16.;
    //     core.world
    //         .set_tile(&"grid".to_string(), i.x as i32, i.y as i32, i.z as i32);
    //     core.world
    //         .get_chunk_mut(i.x as i32, i.y as i32, i.z as i32)
    //         .cook(&core.device);
    // }

    if input_helper.key_released(VirtualKeyCode::Grave) {
        core.global.console = !core.global.console;
        if core.global.console {
            // crate::log::add("$load".to_string());
            core.gui.enable_console()
        } else {
            core.gui.disable_console()
        }
    } else if core.global.console {
        if input_helper.key_released(VirtualKeyCode::Return) {
            let command = crate::log::carriage();
            if command.is_some() {
                // if core.global.test {
                let mut com = command.unwrap(); //.to_lowercase();
                println!("command isss {}", com);
                if let Some(alias) = core.global.aliases.get(&com) {
                    com = alias.to_string();
                }
                for c in com.split("&&") {
                    if !crate::command::init_con_sys(core, c) {
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
                        };

                        crate::lg!("{}", result);
                    }
                }
                // } else {
                //     core.global.test = true;
                // }
            }
        } else if input_helper.key_pressed(VirtualKeyCode::Up) {
            crate::log::history_up()
        } else if input_helper.key_pressed(VirtualKeyCode::Down) {
            crate::log::history_down()
        } else if input_helper.key_held(COMMAND_KEY_L) || input_helper.key_held(COMMAND_KEY_R) {
            if input_helper.key_pressed(VirtualKeyCode::C) {
                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                ctx.set_contents(crate::log::get_line()).unwrap();
            } else if input_helper.key_pressed(VirtualKeyCode::V) {
                let mut ctx: ClipboardContext = ClipboardProvider::new().unwrap();
                match ctx.get_contents() {
                    Ok(s) => crate::log::add(s),
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

                match t.last() {
                    Some(s) => {
                        match s {
                            winit_input_helper::TextChar::Char(c) => match *c as u32 {
                                96 => {}
                                127 => {
                                    crate::log::back();
                                }
                                _ => {
                                    println!("char {}  {}", *c as u32, c);
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
    } else {
        if core.global.debug {
            if input_helper.key_pressed(VirtualKeyCode::Left) {
                core.global.debug_camera_pos.x += 10.;
                lg!("x {}", core.global.debug_camera_pos.x)
            } else if input_helper.key_pressed(VirtualKeyCode::Right) {
                core.global.debug_camera_pos.x -= 10.;
            } else if input_helper.key_pressed(VirtualKeyCode::Up) {
                if input_helper.held_shift() {
                    core.global.debug_camera_pos.z += 10.;
                    lg!("z {}", core.global.debug_camera_pos.z)
                } else {
                    core.global.debug_camera_pos.y += 10.;
                    lg!("y {}", core.global.debug_camera_pos.y)
                }
            } else if input_helper.key_pressed(VirtualKeyCode::Down) {
                if input_helper.held_shift() {
                    core.global.debug_camera_pos.z -= 10.;
                    lg!("z {}", core.global.debug_camera_pos.z)
                } else {
                    core.global.debug_camera_pos.y -= 10.;
                    lg!("y {}", core.global.debug_camera_pos.y)
                }
            }
        }

        if input_helper.key_held(COMMAND_KEY_L) || input_helper.key_held(COMMAND_KEY_R) {
            if input_helper.key_pressed(VirtualKeyCode::R) {
                crate::command::reload(core, core.bundle_manager.console_bundle_target);
            } else if input_helper.key_pressed(VirtualKeyCode::Return) {
                core.global.fullscreen = !core.global.fullscreen;
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

fn log(str: String) {
    crate::log::log(format!("controls::{}", str));
    println!("{}", str);
}

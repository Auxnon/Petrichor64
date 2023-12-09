use crate::log::LogType;
use crate::lua_define::LuaResponse;
use crate::types::ControlState;
use crate::{bundle::BundleManager, Core};
use clipboard::{ClipboardContext, ClipboardProvider};

use winit::event_loop::EventLoopWindowTarget;
use winit::keyboard::PhysicalKey;
use winit::{
    event::{Event, WindowEvent},
    event_loop::ControlFlow,
    keyboard::{Key, KeyCode},
};

#[cfg(target_os = "macos")]
const COMMAND_KEY_L: KeyCode = KeyCode::SuperLeft;
#[cfg(not(target_os = "macos"))]
const COMMAND_KEY_L: KeyCode = KeyCode::ControlLeft;

#[cfg(target_os = "macos")]
const COMMAND_KEY_R: KeyCode = KeyCode::SuperRight;
#[cfg(not(target_os = "macos"))]
const COMMAND_KEY_R: KeyCode = KeyCode::ControlRight;

pub fn controls_evaluate(core: &mut Core, window_target: &mut EventLoopWindowTarget<()>) {
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

    if input_helper.close_requested() {
        window_target.exit();
    }
    match input_helper.dropped_file() {
        // TODO drag and drop
        Some(path) => {
            let s = path.as_os_str().to_string_lossy().to_string();
            if core.global.boot_state {
                core.global.pending_load = Some(s.clone());
            }
            // println!(
            //     "dropped file {} {} {:?}",
            //     s, core.global.boot_state, core.global.pending_load
            // );
            core.bundle_manager.get_main_bundle().lua.call_drop(s)
        }
        _ => {}
    }

    // TODO mouse inputs
    let (mx, my) = input_helper.mouse_diff();
    core.global.game_controller = false;
    core.global.mouse_pos.x = mx / core.gfx.size.width as f32;
    core.global.mouse_pos.y = my / core.gfx.size.height as f32;
    // TODO test if this is not needed on WinOS
    // let diff = input_helper.mouse_diff();
    // core.global.mouse_delta.x = diff.0;
    // core.global.mouse_delta.y = diff.1;

    // println!(core.global.)
    // input_helper.

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

    if input_helper.key_released(KeyCode::Backquote) {
        if !core.global.locked {
            core.global.console = !core.global.console;
            if core.global.console {
                core.gui.enable_console(&core.loggy)
            } else {
                core.gui.disable_console()
            }
        }
    } else if core.global.console {
        if core.global.scroll_delta.1 != 0. {
            core.loggy.scroll(core.global.scroll_delta.1);
        }
        if input_helper.key_released(KeyCode::Enter) {
            let command = core.loggy.carriage();
            if let Some(mut com) = command {
                // println!("command is {}", com);
                crate::core_console_command(core, &mut com);
            }
        } else if input_helper.key_pressed(KeyCode::ArrowUp) {
            core.loggy.history_up()
        } else if input_helper.key_pressed(KeyCode::ArrowDown) {
            core.loggy.history_down()
        } else if input_helper.key_held(COMMAND_KEY_L) || input_helper.key_held(COMMAND_KEY_R) {
            if input_helper.key_pressed(KeyCode::KeyC) {
                if let Ok(mut ctx) = ClipboardContext::new() {
                    if let Err(_) = ctx.set_contents(core.loggy.get_line()) { // TODO
                    }
                }
            } else if input_helper.key_pressed(KeyCode::KeyV) {
                if let Ok(mut ctx) = ClipboardContext::new() {
                    if let Ok(s) = ctx.get_contents() {
                        core.loggy.add(s);
                    }
                }
            } else if input_helper.key_pressed(KeyCode::KeyR) {
                crate::command::reload(core, core.bundle_manager.console_bundle_target);
            } else if input_helper.key_pressed(KeyCode::Escape)
                || input_helper.key_pressed(KeyCode::KeyW)
            {
                window_target.exit();
            } else if input_helper.key_pressed(KeyCode::Enter) {
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
                                // println!("char {}  {}", *c as u32, c);
                                core.loggy.add(String::from(*c))
                            } //st.push(*c),
                        },
                        winit_input_helper::TextChar::Back => {
                            #[cfg(target_os = "windows")]
                            core.loggy.back();
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
            if input_helper.key_pressed(KeyCode::ArrowLeft) {
                core.global.debug_camera_pos.x += 10.;
                core.loggy.log(
                    LogType::Debug,
                    &format!("x {}", core.global.debug_camera_pos.x),
                )
            } else if input_helper.key_pressed(KeyCode::ArrowRight) {
                core.global.debug_camera_pos.x -= 10.;
            } else if input_helper.key_pressed(KeyCode::ArrowUp) {
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
            } else if input_helper.key_pressed(KeyCode::ArrowDown) {
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
            if input_helper.key_pressed(KeyCode::KeyR) {
                crate::command::reload(core, core.bundle_manager.console_bundle_target);
            } else if input_helper.key_pressed(KeyCode::Enter) {
                core.global.fullscreen = !core.global.fullscreen;
                core.check_fullscreen();
                // TODO is it better to have copy within the control handler or leave it up to an app to bind cmd/ctrl+c ?
                // } else if input_helper.key_pressed(KeyCode::C) {
                //     if let Ok(mut ctx) = ClipboardContext::new() {
                //         if let Err(_) = ctx.set_contents(core.loggy.get_line()) { // TODO
                //         }
                //     }
            } else if input_helper.key_pressed(KeyCode::KeyV) {
                if let Ok(mut ctx) = ClipboardContext::new() {
                    if let Ok(s) = ctx.get_contents() {
                        core.bundle_manager.get_main_bundle().lua.call_drop(s);
                    }
                }
            } else if input_helper.key_pressed(KeyCode::KeyW) {
                window_target.exit();
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
                    event:
                        winit::event::KeyEvent {
                            physical_key: PhysicalKey::Code(keycode),
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
                    // KeyCode is an enum with a defined representation
                    // DEV
                    // println!("newkey is {}", *keycode as u32);
                    bits.0[*keycode as usize] = true;
                    match *keycode {
                        KeyCode::AltLeft | KeyCode::AltRight => {
                            bits.0[247] = true;
                        }
                        KeyCode::ControlLeft | KeyCode::ControlRight => {
                            bits.0[248] = true;
                        }
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                            bits.0[249] = true;
                        }
                        KeyCode::SuperLeft | KeyCode::SuperRight => {
                            bits.0[250] = true;
                        }
                        _ => {}
                    }
                }
                winit::event::ElementState::Released => {
                    bits.0[*keycode as usize] = false;
                    match *keycode {
                        KeyCode::AltLeft | KeyCode::AltRight => {
                            bits.0[247] = false;
                        }
                        KeyCode::ControlLeft | KeyCode::ControlRight => {
                            bits.0[248] = false;
                        }
                        KeyCode::ShiftLeft | KeyCode::ShiftRight => {
                            bits.0[249] = false;
                        }
                        KeyCode::SuperLeft | KeyCode::SuperRight => {
                            bits.0[250] = false;
                        }
                        _ => {}
                    }
                }
            }
        }
        _ => {}
    }
    // drop(bits);
}

// fn key_match(key: String) -> Option<KeyCode> {
//     Some(match key.to_lowercase().as_str() {
//         "a" => KeyCode::A,
//         "b" => KeyCode::B,
//         "c" => KeyCode::C,
//         "d" => KeyCode::D,
//         "e" => KeyCode::E,
//         "f" => KeyCode::F,
//         "g" => KeyCode::G,
//         "h" => KeyCode::H,
//         "i" => KeyCode::I,
//         "j" => KeyCode::J,
//         "k" => KeyCode::K,
//         "l" => KeyCode::L,
//         "m" => KeyCode::M,
//         "n" => KeyCode::N,
//         "o" => KeyCode::O,
//         "p" => KeyCode::P,
//         "q" => KeyCode::Q,
//         "r" => KeyCode::R,
//         "s" => KeyCode::S,
//         "t" => KeyCode::T,
//         "u" => KeyCode::U,
//         "v" => KeyCode::V,
//         "w" => KeyCode::W,
//         "x" => KeyCode::X,
//         "y" => KeyCode::Y,
//         "z" => KeyCode::Z,
//         "space" => KeyCode::Space,
//         "lctrl" => KeyCode::LControl,
//         "rctrl" => KeyCode::RControl,
//         _ => return None,
//     })
// }

fn bundle_missing(bm: &BundleManager) -> String {
    format!("please switch to target {} instead", bm.list_bundles())
}

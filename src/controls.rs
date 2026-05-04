use crate::log::LogType;
use crate::types::ControlState;
use crate::{bundle::BundleManager, Core};
use clipboard::{ClipboardContext, ClipboardProvider};

use winit::event::ElementState;
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::KeyCode;

#[cfg(target_os = "macos")]
const COMMAND_KEY_L: KeyCode = KeyCode::SuperLeft;
#[cfg(not(target_os = "macos"))]
const COMMAND_KEY_L: KeyCode = KeyCode::ControlLeft;

#[cfg(target_os = "macos")]
const COMMAND_KEY_R: KeyCode = KeyCode::SuperRight;
#[cfg(not(target_os = "macos"))]
const COMMAND_KEY_R: KeyCode = KeyCode::ControlRight;

/// Evaluate system-level key shortcuts and console input every Lua frame.
///
/// * `bits`      – current key-held state (updated each `window_event`)
/// * `bits_prev` – key state from the previous frame (for pressed/released detection)
///
/// Text typed into the console is handled directly in `window_event` so it
/// arrives before this function runs; only non-text key logic lives here.
pub fn controls_evaluate(
    core: &mut Core,
    window_target: &ActiveEventLoop,
    bits: &ControlState,
    bits_prev: &[bool; 256],
) {
    // Helpers — detect edge transitions from the per-frame key snapshot.
    let key_released =
        |kc: KeyCode| bits_prev[kc as usize] && !bits.0[kc as usize];
    let key_pressed =
        |kc: KeyCode| !bits_prev[kc as usize] && bits.0[kc as usize];
    let key_held = |kc: KeyCode| bits.0[kc as usize];
    // Index 249 is set by bit_check for both ShiftLeft and ShiftRight.
    let held_shift = bits.0[249];
    let held_cmd =
        key_held(COMMAND_KEY_L) || key_held(COMMAND_KEY_R);

    // Backtick toggles the developer console.
    if key_released(KeyCode::Backquote) {
        if !core.global.locked {
            core.global.console = !core.global.console;
            if core.global.console {
                core.gui.enable_console(&core.loggy);
            } else {
                core.gui.disable_console();
            }
        }
    } else if core.global.console {
        // --- Console mode ---

        // Scroll the log with the mouse wheel.
        if core.global.scroll_delta.1 != 0. {
            core.loggy.scroll(core.global.scroll_delta.1);
        }

        if key_released(KeyCode::Enter) {
            let command = core.loggy.carriage();
            if let Some(mut com) = command {
                crate::core_console_command(core, &mut com);
            }
        } else if key_pressed(KeyCode::ArrowUp) {
            core.loggy.history_up();
        } else if key_pressed(KeyCode::ArrowDown) {
            core.loggy.history_down();
        } else if held_cmd {
            if key_pressed(KeyCode::KeyC) {
                if let Ok(mut ctx) = ClipboardContext::new() {
                    let _ = ctx.set_contents(core.loggy.get_line());
                }
            } else if key_pressed(KeyCode::KeyV) {
                if let Ok(mut ctx) = ClipboardContext::new() {
                    if let Ok(s) = ctx.get_contents() {
                        core.loggy.add(&s);
                    }
                }
            } else if key_pressed(KeyCode::KeyR) {
                crate::command::reload(core, core.bundle_manager.console_bundle_target);
            } else if key_pressed(KeyCode::Escape) || key_pressed(KeyCode::KeyW) {
                window_target.exit();
            } else if key_pressed(KeyCode::Enter) {
                core.toggle_fullscreen();
                core.global.fullscreen_state = core.global.fullscreen;
            }
        }
        // Text characters (Key::Character, Space, Backspace) are fed to
        // core.loggy directly in App::window_event so they arrive every
        // KeyboardInput event rather than once per Lua frame.
    } else {
        // --- Game mode ---

        core.global.game_controller = false;

        if core.global.debug {
            if key_pressed(KeyCode::ArrowLeft) {
                core.global.debug_camera_pos.x += 10.;
                core.loggy.log(
                    LogType::Debug,
                    &format!("x {}", core.global.debug_camera_pos.x),
                );
            } else if key_pressed(KeyCode::ArrowRight) {
                core.global.debug_camera_pos.x -= 10.;
            } else if key_pressed(KeyCode::ArrowUp) {
                if held_shift {
                    core.global.debug_camera_pos.z += 10.;
                    core.loggy.log(
                        LogType::Debug,
                        &format!("z {}", core.global.debug_camera_pos.z),
                    );
                } else {
                    core.global.debug_camera_pos.y += 10.;
                    core.loggy.log(
                        LogType::Debug,
                        &format!("y {}", core.global.debug_camera_pos.y),
                    );
                }
            } else if key_pressed(KeyCode::ArrowDown) {
                if held_shift {
                    core.global.debug_camera_pos.z -= 10.;
                    core.loggy.log(
                        LogType::Debug,
                        &format!("z {}", core.global.debug_camera_pos.z),
                    );
                } else {
                    core.global.debug_camera_pos.y -= 10.;
                    core.loggy.log(
                        LogType::Debug,
                        &format!("y {}", core.global.debug_camera_pos.y),
                    );
                }
            }
        }

        if held_cmd {
            if key_pressed(KeyCode::KeyR) {
                crate::command::reload(core, core.bundle_manager.console_bundle_target);
            } else if key_pressed(KeyCode::Enter) {
                core.global.fullscreen = !core.global.fullscreen;
                core.check_fullscreen();
            } else if key_pressed(KeyCode::KeyV) {
                if let Ok(mut ctx) = ClipboardContext::new() {
                    if let Ok(s) = ctx.get_contents() {
                        core.bundle_manager.get_main_bundle().lua.call_drop(s);
                    }
                }
            } else if key_pressed(KeyCode::KeyW) {
                window_target.exit();
            }
        }
    }
}

/// Update the boolean key-state array from a single `KeyboardInput` event.
pub fn bit_check(state: &ElementState, keycode: KeyCode, bits: &mut ControlState) {
    match state {
        ElementState::Pressed => {
            bits.0[keycode as usize] = true;
            match keycode {
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
        ElementState::Released => {
            bits.0[keycode as usize] = false;
            match keycode {
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

fn bundle_missing(bm: &BundleManager) -> String {
    format!("please switch to target {} instead", bm.list_bundles())
}

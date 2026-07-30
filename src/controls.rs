use crate::log::LogType;
use crate::types::ControlState;
use crate::{bundle::BundleManager, Core};
#[cfg(not(target_arch = "wasm32"))]
#[cfg(desktop)]
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
    // Read through the same KeyCode->legacy-index map bit_check writes with, so
    // internal checks and Lua's key()/key_match agree (else e.g. Backquote's
    // discriminant 0 collides with Digit1's legacy slot 0 — pressing "1" would
    // toggle the console).
    let key_released =
        |kc: KeyCode| keycode_to_index(kc).map_or(false, |i| bits_prev[i] && !bits.0[i]);
    let key_pressed =
        |kc: KeyCode| keycode_to_index(kc).map_or(false, |i| !bits_prev[i] && bits.0[i]);
    let key_held = |kc: KeyCode| keycode_to_index(kc).map_or(false, |i| bits.0[i]);
    // Index 249 is set by bit_check for both ShiftLeft and ShiftRight.
    let held_shift = bits.0[249];
    let held_cmd = key_held(COMMAND_KEY_L) || key_held(COMMAND_KEY_R);

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
                #[cfg(desktop)]
                if let Ok(mut ctx) = ClipboardContext::new() {
                    let _ = ctx.set_contents(core.loggy.get_line());
                }
            } else if key_pressed(KeyCode::KeyV) {
                #[cfg(desktop)]
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
                #[cfg(desktop)]
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
/// Translate a winit `KeyCode` into the engine's legacy key index (the scheme
/// `command::key_match` / `key_unmatch` use, which mirrors the old winit
/// `VirtualKeyCode` order). winit 0.29 replaced `VirtualKeyCode` with physical
/// `KeyCode`, whose discriminants are ordered completely differently, so writing
/// `keycode as usize` landed keys in the wrong slots — `key()`/`cin()` only
/// worked where the two happened to coincide. Returns `None` for keys outside
/// the mapped set.
fn keycode_to_index(kc: KeyCode) -> Option<usize> {
    use KeyCode::*;
    Some(match kc {
        Digit1 => 0, Digit2 => 1, Digit3 => 2, Digit4 => 3, Digit5 => 4,
        Digit6 => 5, Digit7 => 6, Digit8 => 7, Digit9 => 8, Digit0 => 9,
        KeyA => 10, KeyB => 11, KeyC => 12, KeyD => 13, KeyE => 14, KeyF => 15,
        KeyG => 16, KeyH => 17, KeyI => 18, KeyJ => 19, KeyK => 20, KeyL => 21,
        KeyM => 22, KeyN => 23, KeyO => 24, KeyP => 25, KeyQ => 26, KeyR => 27,
        KeyS => 28, KeyT => 29, KeyU => 30, KeyV => 31, KeyW => 32, KeyX => 33,
        KeyY => 34, KeyZ => 35,
        Escape => 36,
        F1 => 37, F2 => 38, F3 => 39, F4 => 40, F5 => 41, F6 => 42, F7 => 43,
        F8 => 44, F9 => 45, F10 => 46, F11 => 47, F12 => 48, F13 => 49, F14 => 50,
        F15 => 51, F16 => 52, F17 => 53, F18 => 54, F19 => 55, F20 => 56, F21 => 57,
        F22 => 58, F23 => 59, F24 => 60,
        PrintScreen => 61,
        Delete => 66, End => 67, PageDown => 68, PageUp => 69,
        ArrowLeft => 70, ArrowUp => 71, ArrowRight => 72, ArrowDown => 73,
        Backspace => 74, Enter => 75, Space => 76,
        // Engine-internal keys (not exposed by name via key_match): the console
        // toggle and modifiers. 62 is free in the legacy scheme; the modifiers
        // share the same slots bit_check's explicit match uses (247..=250).
        Backquote => 62,
        AltLeft | AltRight => 247,
        ControlLeft | ControlRight => 248,
        ShiftLeft | ShiftRight => 249,
        SuperLeft | SuperRight => 250,
        _ => return None,
    })
}

pub fn bit_check(state: &ElementState, keycode: KeyCode, bits: &mut ControlState) {
    match state {
        ElementState::Pressed => {
            if let Some(i) = keycode_to_index(keycode) {
                bits.0[i] = true;
            }
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
            if let Some(i) = keycode_to_index(keycode) {
                bits.0[i] = false;
            }
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

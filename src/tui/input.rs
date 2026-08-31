use crate::types::ControlState;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::Duration;

/// Map a crossterm key to the engine's legacy key index — the same scheme
/// `controls::keycode_to_index` uses for winit `KeyCode`, duplicated here
/// since crossterm and winit define unrelated `KeyCode` enums with no shared
/// type to dispatch on. Keeps Lua's `key()`/`cin()` behaving the same way
/// under the TUI backend as under the windowed one.
fn key_to_index(kc: KeyCode) -> Option<usize> {
    Some(match kc {
        KeyCode::Char(c) => match c.to_ascii_uppercase() {
            '1' => 0, '2' => 1, '3' => 2, '4' => 3, '5' => 4,
            '6' => 5, '7' => 6, '8' => 7, '9' => 8, '0' => 9,
            'A' => 10, 'B' => 11, 'C' => 12, 'D' => 13, 'E' => 14, 'F' => 15,
            'G' => 16, 'H' => 17, 'I' => 18, 'J' => 19, 'K' => 20, 'L' => 21,
            'M' => 22, 'N' => 23, 'O' => 24, 'P' => 25, 'Q' => 26, 'R' => 27,
            'S' => 28, 'T' => 29, 'U' => 30, 'V' => 31, 'W' => 32, 'X' => 33,
            'Y' => 34, 'Z' => 35,
            ' ' => 76,
            '`' => 62,
            _ => return None,
        },
        KeyCode::Esc => 36,
        KeyCode::Backspace => 74,
        KeyCode::Enter => 75,
        KeyCode::Left => 70,
        KeyCode::Up => 71,
        KeyCode::Right => 72,
        KeyCode::Down => 73,
        _ => return None,
    })
}

/// Terminal keyboard state feeding the same `ControlState` bitset the
/// windowed backend's `bit_check` produces, so `Core::update`/Lua's `key()`
/// see identical input shapes regardless of backend.
pub struct TuiInput {
    bits: ControlState,
}

impl TuiInput {
    pub fn new() -> Self {
        Self {
            bits: ControlState::default(),
        }
    }

    /// Drain pending terminal input events (non-blocking). Returns `true` if
    /// Escape was pressed, signalling the caller should quit.
    ///
    /// Most terminals don't report key-release events unless the caller opts
    /// into the Kitty keyboard protocol, so held keys track OS auto-repeat
    /// rather than a true press/release edge — adequate for movement testing,
    /// not a perfect substitute for the windowed backend's edge detection.
    pub fn poll(&mut self) -> bool {
        let mut quit = false;
        while matches!(event::poll(Duration::from_secs(0)), Ok(true)) {
            let Ok(Event::Key(key)) = event::read() else {
                continue;
            };
            match key.kind {
                KeyEventKind::Release => {
                    if let Some(i) = key_to_index(key.code) {
                        self.bits.0[i] = false;
                    }
                }
                KeyEventKind::Press | KeyEventKind::Repeat => {
                    if let Some(i) = key_to_index(key.code) {
                        self.bits.0[i] = true;
                    }
                    if key.code == KeyCode::Esc {
                        quit = true;
                    }
                }
            }
        }
        quit
    }

    pub fn state(&self) -> &ControlState {
        &self.bits
    }
}

use crate::types::ControlState;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use std::time::{Duration, Instant};

/// A held key with no Repeat/Release for this long is treated as released.
/// Terminals without the Kitty keyboard protocol never send a real release —
/// OS auto-repeat re-sends Press/Repeat at some delay+rate we don't control,
/// so this has to clear well past a typical repeat gap (rates seen in the
/// wild: ~25-40ms after an initial ~300-500ms delay) or a still-held key
/// would flicker released between repeats. 400ms clears a truly-released key
/// quickly while comfortably outlasting that gap.
const HOLD_TIMEOUT: Duration = Duration::from_millis(400);

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
    /// Last time each legacy key index saw a Press/Repeat, so a held key with
    /// no real Release (see `poll`) can still be timed out — `None` once
    /// released (real or timed-out), so an already-clear key is never rechecked.
    last_seen: [Option<Instant>; 256],
}

impl TuiInput {
    pub fn new() -> Self {
        Self {
            bits: ControlState::default(),
            last_seen: [None; 256],
        }
    }

    /// Drain pending terminal input events (non-blocking), then release any
    /// key that's gone quiet past `HOLD_TIMEOUT`. Returns `true` if Escape
    /// was pressed, signalling the caller should quit.
    ///
    /// Most terminals don't report key-release events unless the caller opts
    /// into the Kitty keyboard protocol, so held keys track OS auto-repeat
    /// rather than a true press/release edge. Without the timeout sweep below
    /// this made a key that really did release **never** depress again
    /// (`bits.0[i]` set true on the one Press it got and nothing downstream
    /// ever cleared it) — adequate for a quick smoke test, not for anything
    /// that runs longer than one keypress.
    pub fn poll(&mut self) -> bool {
        let mut quit = false;
        let now = Instant::now();
        while matches!(event::poll(Duration::from_secs(0)), Ok(true)) {
            let Ok(Event::Key(key)) = event::read() else {
                continue;
            };
            match key.kind {
                KeyEventKind::Release => {
                    if let Some(i) = key_to_index(key.code) {
                        self.bits.0[i] = false;
                        self.last_seen[i] = None;
                    }
                }
                KeyEventKind::Press | KeyEventKind::Repeat => {
                    if let Some(i) = key_to_index(key.code) {
                        self.bits.0[i] = true;
                        self.last_seen[i] = Some(now);
                    }
                    if key.code == KeyCode::Esc {
                        quit = true;
                    }
                }
            }
        }
        for i in 0..256 {
            if let Some(seen) = self.last_seen[i] {
                if now.duration_since(seen) > HOLD_TIMEOUT {
                    self.bits.0[i] = false;
                    self.last_seen[i] = None;
                }
            }
        }
        quit
    }

    pub fn state(&self) -> &ControlState {
        &self.bits
    }
}

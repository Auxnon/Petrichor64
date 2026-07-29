//! MIDI input (the `midi` feature, native-only).
//!
//! Wraps `midir`, which binds the host's native MIDI stack — CoreMIDI on macOS,
//! WinMM/WinRT on Windows, ALSA sequencer on Linux. That's also how **BLE MIDI**
//! works here without a line of Bluetooth code: macOS and Windows present an
//! OS-paired Bluetooth MIDI device as an ordinary MIDI port, so pairing it once in
//! the OS is enough for us to see it. (Linux's BlueZ does not bridge BLE MIDI into
//! ALSA, so Bluetooth devices won't appear there — a direct-BLE backend via
//! `btleplug` is the fix, and is deliberately not part of this feature yet.)
//!
//! Incoming notes are played by the synth directly: the input callback translates
//! MIDI bytes into `SoundCommand`s and pushes them onto the same `mpsc` the
//! engine already uses (`core.singer`), so MIDI is just another producer on the
//! mixer's queue and lands sample-accurately. Games can additionally read the raw
//! stream from Lua via `midi()`.
//!
//! **State lives in module statics on purpose.** A MIDI port is a single global
//! hardware resource (like the keyboard), not per-bundle state, and the
//! connection handle simply has to outlive the call that opened it. Keeping it
//! here means opening/polling needs no plumbing through `Core`, the VM
//! construction path, or the worker. Like the audio stream, the connection
//! **survives Lua reloads**; `reset()` drops the queued events (see
//! `command.rs::async_load_app`) but leaves the hardware connected.

use std::collections::VecDeque;
use std::sync::{Mutex, OnceLock};

use midir::{MidiInput, MidiInputConnection};

use crate::lua_define::SoundSender;
use crate::sound::{Note, SoundCommand};

/// Cap on buffered events, so a game that never calls `midi()` can't grow the
/// queue without bound. Oldest events are dropped first.
const MAX_EVENTS: usize = 256;

/// How long a MIDI note sustains if its note-off never arrives (a dropped BLE
/// packet, or a controller unplugged mid-note). Long enough to feel like a real
/// sustain, short enough that a lost note-off doesn't hang forever.
const MAX_SUSTAIN_SECS: f32 = 30.0;

/// One raw MIDI event, as handed to Lua: `(status, channel, data1, data2)`.
/// `status` is the message's high nibble (0x90 note-on, 0x80 note-off, 0xB0
/// control-change, …) and `channel` its low nibble.
pub type MidiEvent = (u8, u8, u8, u8);

/// The live connection. Held only to keep it open — dropping it closes the port.
static CONN: Mutex<Option<MidiInputConnection<()>>> = Mutex::new(None);
/// Name of the currently connected port, if any.
static PORT: Mutex<Option<String>> = Mutex::new(None);

/// Events waiting for Lua to drain via `midi()`.
fn events() -> &'static Mutex<VecDeque<MidiEvent>> {
    static EVENTS: OnceLock<Mutex<VecDeque<MidiEvent>>> = OnceLock::new();
    EVENTS.get_or_init(|| Mutex::new(VecDeque::new()))
}

/// Equal-temperament MIDI note number → Hz (69 = A4 = 440). Both note-on and
/// note-off run this, so `ReleaseNote` can match on the frequency.
pub fn note_freq(note: u8) -> f32 {
    440.0 * 2f32.powf((note as f32 - 69.0) / 12.0)
}

/// Names of every available MIDI input port, in index order. Cheap and callable
/// from any thread — it opens a throwaway client just to enumerate.
pub fn ports() -> Vec<String> {
    match MidiInput::new("petrichor64-scan") {
        Ok(input) => input
            .ports()
            .iter()
            .map(|p| input.port_name(p).unwrap_or_else(|_| "?".to_string()))
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// The port we're currently connected to, if any.
pub fn connected() -> Option<String> {
    PORT.lock().ok().and_then(|p| p.clone())
}

/// Open a MIDI input port and start feeding the synth. `want` picks a port by
/// (case-insensitive) substring of its name; `None` takes the first available.
/// Returns the connected port's name, or an error string. Opening replaces any
/// existing connection.
pub fn open(singer: SoundSender, want: Option<&str>) -> Result<String, String> {
    let mut input = MidiInput::new("petrichor64").map_err(|e| e.to_string())?;
    // Ignore active-sensing/timing-clock spam; we don't use it and it would
    // dominate the event queue.
    input.ignore(midir::Ignore::All);

    let ports = input.ports();
    if ports.is_empty() {
        return Err("no midi input ports".to_string());
    }
    let port = match want {
        Some(name) => {
            let lower = name.to_lowercase();
            ports
                .iter()
                .find(|p| {
                    input
                        .port_name(p)
                        .map(|n| n.to_lowercase().contains(&lower))
                        .unwrap_or(false)
                })
                .ok_or_else(|| format!("no midi port matching '{}'", name))?
                .clone()
        }
        None => ports[0].clone(),
    };
    let name = input.port_name(&port).map_err(|e| e.to_string())?;

    // Drop any previous connection *before* opening the new one.
    close();

    let conn = input
        .connect(
            &port,
            "petrichor64-in",
            move |_stamp, message, _| handle_message(message, &singer),
            (),
        )
        .map_err(|e| e.to_string())?;

    if let Ok(mut slot) = CONN.lock() {
        *slot = Some(conn);
    }
    if let Ok(mut slot) = PORT.lock() {
        *slot = Some(name.clone());
    }
    Ok(name)
}

/// Close the connection (releasing the port), leaving queued events alone.
pub fn close() {
    if let Ok(mut slot) = CONN.lock() {
        slot.take(); // dropping the connection closes the port
    }
    if let Ok(mut slot) = PORT.lock() {
        *slot = None;
    }
}

/// Take every queued event, emptying the queue.
pub fn drain() -> Vec<MidiEvent> {
    match events().lock() {
        Ok(mut q) => q.drain(..).collect(),
        Err(_) => Vec::new(),
    }
}

/// App (re)load: drop stale events and silence anything still sounding, but keep
/// the port open — the hardware outlives a Lua reload, like the audio stream.
pub fn reset(singer: &SoundSender) {
    if let Ok(mut q) = events().lock() {
        q.clear();
    }
    let _ = singer.send(SoundCommand::Stop(None));
}

/// Translate one incoming MIDI message: drive the synth, and queue the raw event
/// for Lua. Runs on midir's own callback thread, so it only does lock-free sends
/// and a `try_lock` on the event queue — it never blocks or allocates a buffer.
fn handle_message(message: &[u8], singer: &SoundSender) {
    if message.is_empty() {
        return;
    }
    let status = message[0] & 0xF0;
    let channel = message[0] & 0x0F;
    let d1 = message.get(1).copied().unwrap_or(0);
    let d2 = message.get(2).copied().unwrap_or(0);

    // Route each MIDI channel to the matching synth channel, so a multi-timbral
    // controller lands on separate tracks (with their own lanes and effects).
    let ch = Some(channel as usize);
    match status {
        // Note-on with velocity 0 is the conventional note-off.
        0x90 if d2 > 0 => {
            let freq = note_freq(d1);
            let volume = (d2 as f32 / 127.0).clamp(0.0, 1.0);
            // Sustain until note-off (bounded, in case that never arrives).
            let note = Note::new(0, freq, MAX_SUSTAIN_SECS, volume);
            let _ = singer.send(SoundCommand::PlayNote(note, ch));
        }
        0x80 | 0x90 => {
            let _ = singer.send(SoundCommand::ReleaseNote(ch, note_freq(d1)));
        }
        // All-notes-off (123) / all-sound-off (120): the panic buttons.
        0xB0 if d1 == 120 || d1 == 123 => {
            let _ = singer.send(SoundCommand::Stop(ch));
        }
        _ => {}
    }

    // Queue for Lua. `try_lock` so a contended frame drops the event rather than
    // stalling the MIDI thread.
    if let Ok(mut q) = events().try_lock() {
        if q.len() >= MAX_EVENTS {
            q.pop_front();
        }
        q.push_back((status, channel, d1, d2));
    }
}

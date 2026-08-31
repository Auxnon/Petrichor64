## midi

_MIDI input: read a controller, or let it play the synth_

```lua
---@type fun(action?: string, name?: string): table|string|nil
function midi(action, name)
```

MIDI input is a **build feature** (`--features midi`) and is **off by default** —
MIDI is a niche for a fantasy console, so its cost stays opt-in. Native only for
now (see below).

**It works without any Lua at all.** At boot the engine connects to the first
available MIDI input port, and incoming notes play the synth: note-on starts a
note that **sustains until note-off** (velocity sets its volume), and each MIDI
channel routes to the matching engine channel, so a multi-timbral controller lands
on separate tracks with their own lanes and effects. Controller messages
"all sound off" (CC 120) and "all notes off" (CC 123) stop a channel.

### Bluetooth (BLE MIDI)

Works out of the box on **macOS and Windows**: those systems present an OS-paired
Bluetooth MIDI device as an ordinary MIDI port, so pair it once and the engine
sees it like any other controller.

- **macOS** — Audio MIDI Setup → Window → Show MIDI Studio → Bluetooth
- **Windows** — Settings → Bluetooth & devices → Add device

**Linux** is the exception: BlueZ doesn't bridge BLE MIDI into the ALSA sequencer,
so Bluetooth controllers won't show up there (USB and DIN work fine).

### Reading events from Lua

Call `midi()` each frame to drain the queued events. It returns an array of
`{status, channel, data1, data2}` — `status` is the message type (`0x90` note-on,
`0x80` note-off, `0xB0` control-change), `channel` is `0..15`, and the two data
bytes are usually note + velocity, or CC number + value.

```lua
for _, e in ipairs(midi()) do
    local status, chan, d1, d2 = e[1], e[2], e[3], e[4]
    if status == 0x90 and d2 > 0 then
        print('note on ' .. d1 .. ' vel ' .. d2)   -- also already sounding
    elseif status == 0xB0 then
        filt(0, 'low', 200 + d2 * 30)              -- map a knob to the filter
    end
end
```

Events are queued between calls (up to 256, oldest dropped first), so a frame that
skips the call won't lose much — but drain it every frame if you're reading input.

### Ports

```lua
midi('ports')          -- { 'Launchkey MINI', 'IAC Driver Bus 1' }
midi('port')           -- 'Launchkey MINI'  (the connected one, or nil)
midi('open', 'launch') -- connect by name substring, case-insensitive
midi('open')           -- connect to the first available port
midi('close')          -- disconnect
```

`midi('open', …)` returns the connected port's name, or `nil` on failure.

Notes: the connection **survives a game reload** (like the audio stream) — the
queue is cleared and sounding notes are stopped, but the port stays open. A
note-off that never arrives (an unplugged controller, a dropped Bluetooth packet)
releases on its own after 30 seconds; `mute()` is the immediate panic button.

Not yet supported: MIDI **output** (driving external gear), and MIDI on the
**web** build — `midir` does have a Web MIDI backend, but routing it through the
wasm VM worker is a separate job.

See also `note`, `mute`, `filt`, `attr`.

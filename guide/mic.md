## mic

_record a microphone snippet straight into a sample slot_

```lua
---@type fun(id?: integer, secs?: number): boolean
function mic(id, secs)
```

Captures a short snippet from the system's default audio **input** device and
turns it into a playable instrument — the sampler trick from hardware like the
PO-33: grab a sound, then play it back as notes.

- `id` — the sample/instrument slot to record into, exactly like `smpl`. Once the
  capture lands, `note(440, len, ch, id)` plays it back at its natural speed
  (`440` is the base pitch the recording is bound at; higher notes play it faster
  and pitched up, lower notes slower and down).
- `secs` — capture length in seconds. Default `1`, maximum `10`.
- Returns `true` if recording started; `false` if there's no input device or a
  capture is already in flight.
- `mic()` with **no arguments** doesn't record — it returns `true` while a capture
  is running, so you can wait for the sample to be ready.

```lua
if key('r', true) then
    mic(3, 2)                    -- record 2 seconds into slot 3
end

if not mic() and key('space', true) then
    note(440, 1, 0, 3)           -- play the recording back
    note(660, 1, 0, 3)           -- ...and pitched up a fifth
end
```

The capture is downmixed to mono, loudness-matched like every other sample, and
tagged with the input device's own sample rate, so playback pitch is right even
when your input and output devices run at different rates.

It's also filed in the loaded-sound bank under the name `'mic'`, so the last
recording can be bound to more slots with different settings:

```lua
mic(1, 1)                        -- record
smpl(2, 'mic', { base = 220 })   -- same audio, another slot, an octave down
```

Then treat it like any sample — chop it up with envelopes, or run it through the
channel effects:

```lua
note(440, 0.4, 2, 1)
crsh(2, 5, 8000)                 -- crunchy lo-fi sampler
verb(2, 0.6)
```

### Privacy

The engine **never opens the microphone on its own** — only an explicit `mic(id)`
call starts a capture, and the input stream is closed the moment the snippet
lands (so your OS's "microphone in use" indicator goes out immediately rather than
staying lit). The first capture may raise your OS's microphone permission prompt.

Native only for now — the web build has no microphone path (that would need
`getUserMedia`). See also `smpl`, `note`, `crsh`, `verb`.

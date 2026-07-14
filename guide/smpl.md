## smpl

_define an instrument from raw PCM samples_

```lua
---@type fun(id: integer, data: number[], base?: number)
function smpl(id, data, base)
```

Registers instrument `id` as a **sampler**: instead of an oscillator, notes play
back the raw waveform in `data` (samples in `-1..1`), resampled to whatever pitch
the note asks for. `base` is the frequency the sample was recorded at — a note of
that frequency plays the sample untouched; higher notes speed it up, lower notes
slow it down. `base` defaults to `440` (A4).

The sample is a **one-shot**: it plays through once and then releases, regardless
of the note's requested length (a longer length just holds the tail of silence).
For a percussive hit, keep `data` short; for a sustained tone, record a full
cycle-rich buffer at `base`.

This is the retro, pre-PS1 way to get "real" instrument timbres cheaply — bake a
short buffer once, then pitch it around the keyboard.

```lua
-- A quick decaying "pluck": a sine that fades over ~4000 samples, recorded at 440.
local buf = {}
local n = 4000
for i = 1, n do
    local t = (i - 1) / 44100          -- assume 44.1kHz
    local decay = 1 - (i - 1) / n      -- linear fade to silence
    buf[i] = sin(t * 440 * tau) * decay
end
smpl(1, buf, 440)
note(523.25, 0.5, nil, 1)  -- plays the pluck up a fifth-ish (C5)
```

See also `instr` (oscillator / additive instruments), `note`, `chord`, `song`.

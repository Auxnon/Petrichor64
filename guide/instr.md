## instr

_define an instrument_

```lua
---@type fun(id: integer, spec: string|number[], cfg?: table)
function instr(id, spec, cfg)
```

Defines instrument `id`, later selected by `note`/`song`/`chord`. `spec` is
either a **waveform name** (a direct oscillator) or a **harmonic-amplitude
table** (additive synthesis):

- `'square'` (or `'sqr'`) — hollow, chiptune square. The default for notes that
  don't name an instrument.
- `'pulse'` — pulse/PWM; set the duty cycle with `cfg.wid` (0..1, 0.5 = square).
- `'saw'` — bright, buzzy sawtooth.
- `'tri'` (or `'triangle'`) — soft, flute-ish triangle.
- `'sine'` — pure tone.
- `'noise'` — white noise (percussion / hats).
- `{a1, a2, a3, …}` — additive: amplitude of each harmonic (1st, 2nd, 3rd, …).

### cfg (optional): duty + ADSR envelope

The trailing `cfg` table shapes the instrument (all keys optional):

- `wid` — pulse duty cycle 0..1 (only used by `'pulse'`).
- `atk` — attack time in seconds (ramp up from silence). Default `0.004`.
- `dec` — decay time in seconds (fall from peak to `sus`). Default `0` (none).
- `sus` — sustain level 0..1, held while the note sounds. Default `1`.
- `rel` — release time in seconds (fade after the note ends). Default `0.012`.

The envelope belongs to the instrument, so `note`/`chord`/`song` stay terse —
just reference the id. (A bare number as `cfg` is still read as the pulse duty,
for back-compat: `instr(id, 'pulse', 0.25)`.)

```lua
instr(1, 'pulse', { wid = 0.25 })          -- thin pulse lead
instr(2, 'saw', { atk = .01, dec = .1, sus = .6, rel = .3 }) -- plucky swell
instr(3, 'noise', { atk = 0, rel = .05 })  -- tight noise hit
instr(4, { 1, 0, 0.5, 0, 0.3 })            -- odd-harmonic organ-ish tone
note(440, 0.5, nil, 1) -- play A4 on channel-auto with instrument 1
```

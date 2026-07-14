## instr

_define an instrument_

```lua
---@type fun(id: integer, spec: string|number[], width?: number)
function instr(id, spec, width)
```

Defines instrument `id`, later selected by `note`/`song`/`chord`. `spec` is
either a **waveform name** (a direct oscillator) or a **harmonic-amplitude
table** (additive synthesis):

- `'square'` (or `'sqr'`) — hollow, chiptune square. The default for notes that
  don't name an instrument.
- `'pulse'` + `width` — pulse/PWM, `width` is the duty cycle 0..1 (0.5 = square).
- `'saw'` — bright, buzzy sawtooth.
- `'tri'` (or `'triangle'`) — soft, flute-ish triangle.
- `'sine'` — pure tone.
- `'noise'` — white noise (percussion / hats).
- `{a1, a2, a3, …}` — additive: amplitude of each harmonic (1st, 2nd, 3rd, …).

```lua
instr(1, 'pulse', 0.25) -- thin pulse lead
instr(2, 'saw')
instr(3, 'noise')
instr(4, { 1, 0, 0.5, 0, 0.3 }) -- odd-harmonic organ-ish tone
note(440, 0.5, nil, 1) -- play A4 on channel-auto with instrument 1
```

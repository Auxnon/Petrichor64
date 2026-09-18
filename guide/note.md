## note

_play note_

```lua
---@type fun(freq: number, length?: number, channel?: integer, instrument?: integer)
function note(freq, length, channel, instrument)
```

Play a note of the specified frequency in hertz with an optional `length`
(seconds), on an optional `channel` with an optional `instrument`. `440.0` is A
above middle C. All sound runs on its own thread independent of the game loop.

- `channel` — which channel ("track") to play on. Default `0`. A channel is
  polyphonic: overlapping notes on the same channel sound together as a **chord**
  (up to the channel's lane count — see `attr{lanes}`). Use different channels for
  independent voices/effects.
- `instrument` — an instrument id defined with `instr`/`smpl`. Default `0`.

```lua
note(110.0)             -- play A1 on channel 0
note(440, 0.5, 0, 2)    -- A4 for 0.5s on channel 0 with instrument 2

-- a chord: three notes on one channel sound together
note(261.63, 1, 0)
note(329.63, 1, 0)
note(392.00, 1, 0)
```

See also `chord`, `song`, `sing`, `fade`, `attr`, `instr`.

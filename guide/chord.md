## chord

_play several notes at once_

```lua
---@type fun(freqs: number[], length?: number, instrument?: integer)
function chord(freqs, length, instrument)
```

Play multiple frequencies simultaneously. Each frequency is voiced on its own
free channel, so they sound together rather than in sequence. `length` is in
seconds (default 1), and `instrument` selects a table defined with `instr`
(default 0, the built-in square-ish tone).

```lua
chord({ 261.6, 329.6, 392.0 })      -- a C major triad for 1s
chord({ 220, 277.2, 329.6 }, 0.5)   -- an A major triad for half a second
```

See also [note](note.md) (single note, optional channel) and [song](song.md)
(a sequence on one channel).

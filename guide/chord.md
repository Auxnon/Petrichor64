## chord

_play several notes at once_

```lua
---@type fun(freqs: number[], length?: number, instrument?: integer)
function chord(freqs, length, instrument)
```

Play multiple frequencies simultaneously. Each frequency takes a free lane of
channel 0, so they sound together rather than in sequence (raise channel 0's lane
count with `attr{lanes}` if a chord is bigger than the default 8). `length` is in
seconds (default 1), and `instrument` selects a table defined with `instr`
(default 0, the built-in square-ish tone).

```lua
chord({ 261.6, 329.6, 392.0 })      -- a C major triad for 1s
chord({ 220, 277.2, 329.6 }, 0.5)   -- an A major triad for half a second
```

See also [note](note.md) (single note, optional channel) and [song](song.md)
(a sequence on one channel).

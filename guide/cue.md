## cue

_play a note on the transport_

```lua
---@type fun(freq:number, length?:number, channel?:integer, instrument?:integer, at?:number, grid?:number)
function cue(freq, length, channel, instrument, at, grid)
```

Like `note`, but the note starts at a point on the transport rather than the moment
the call is made. `note` fires on arrival, so its timing is quantized to whichever
game frame sent it — 16ms of jitter, and two beats started on different frames stay
offset forever. `cue` hands the beat to the audio thread and lets the sample clock
decide, which is what makes a rhythm steady.

`at` is an absolute transport beat. It is clamped forward, so a beat already gone
plays immediately rather than being dropped.

`grid` snaps `at` up to the next multiple of itself: `1` is the next whole beat,
`0.25` the next sixteenth. This is the useful one for a game — press a key at any
moment and the sound lands on the grid.

```lua
bpm(120)
cue(440, 0.5, 0, 0, 4)          -- exactly on beat 4
cue(220, 0.25, 1, 0, 0, 0.25)   -- on the next sixteenth, whenever you called it
```

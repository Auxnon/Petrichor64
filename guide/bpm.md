## bpm

_set the transport tempo_

```lua
---@type fun(tempo:number)
function bpm(tempo)
```

Sets the tempo of the shared transport, in beats per minute. Every channel reads the
same clock: one transport with per-channel subdivisions is what gives polyrhythm,
whereas independent tempos per channel would only give drift. Default is 120.

Tempo is applied on the audio thread, which also owns the sample counter, so changing
it cannot race a note already scheduled with `cue`.

```lua
bpm(174) -- jungle
```

## verb

_add reverb to a channel (a channel-level effect)_

```lua
---@type fun(channel: integer, room: number, damp?: number, wet?: number)
function verb(channel, room, damp, wet)
```

Adds **reverb** — a sense of space/room — to a playback **channel**. It's a
compact Freeverb (parallel damped comb filters into a diffusing all-pass bank),
run over the channel's whole mixed output, so every note/`sing`/`smpl` on it sits
in the same space and the tail rings out after the notes stop. Like `fade`/`echo`/
`filt`, it's a channel-level effect, so a chord reverberates as a whole.

- `channel` — the channel to reverb (0-based, as in `note`/`fade`).
- `room` — tail length / decay, `0..1`. Small = a tight room, large = a long hall.
  `room <= 0` **disables** the reverb (and frees its buffers).
- `damp` — how much the tail's high frequencies roll off, `0..1`. `0` = bright,
  higher = darker/warmer tail. Default `0.5`.
- `wet` — reverb level mixed on top of the dry signal. Default `0.3`.

```lua
note(330, 0.4, 4, 1)   -- a blip on channel 4
verb(4, 0.6)           -- a medium room

-- a big, dark hall for a sung phrase
verb(5, 0.9, 0.7, 0.5)
sing('aa', 220, 1.5, 5)

verb(4, 0)             -- reverb off
```

Runs last in the channel effect chain (**filt → echo → verb → fade**), so it adds
space to the filtered, echoed signal. See also `echo`, `filt`, `fade`, `sing`, `attr`.

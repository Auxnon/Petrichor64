## fade

_fade a channel's volume (a channel-level effect)_

```lua
---@type fun(channel: integer, secs: number, target?: number)
function fade(channel, secs, target)
```

Ramps a playback **channel's** output gain to `target` over `secs` seconds, using
a smooth crossfade. This is the first channel-level *effect* — the same
`Crossfade` primitive that blends a sung consonant into its vowel, applied across
a whole channel.

- `channel` — the channel to fade (0–15).
- `secs` — fade duration in seconds.
- `target` — target gain, `0..1`. Default `0` (fade to silence). `1` fades in.

Notes on that channel keep playing; only their combined level is scaled. Play a
note/`sing`/`smpl` on an explicit channel so you know which one to fade.

```lua
note(440, 4, 3, 1)   -- hold a tone on channel 3
fade(3, 2)           -- fade channel 3 out over 2s (target defaults to 0)

-- crossfade two channels: A out while B in
fade(1, 1.5, 0)      -- channel 1 down
fade(2, 1.5, 1)      -- channel 2 up
```

More effects (filter sweeps, delay, …) will layer onto channels the same way as
the effects system grows. See also `mute`, `note`, `sing`.

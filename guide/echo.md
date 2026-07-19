## echo

_add a feedback-delay echo to a channel (a channel-level effect)_

```lua
---@type fun(channel: integer, secs: number, feedback?: number, mix?: number)
function echo(channel, secs, feedback, mix)
```

Adds an **echo** (feedback delay) to a playback **channel**. It processes the
channel's whole mixed output, so every note/`sing`/`smpl` on that channel picks up
the same echo — and the echoes keep ringing out after the notes stop. Like `fade`,
it's a channel-level effect, so a chord (which sounds across one channel's lanes)
echoes as a whole.

- `channel` — the channel to echo (0-based, as in `note`/`fade`).
- `secs` — delay time between echoes, in seconds. `secs <= 0` **disables** the
  echo (and clears its tail).
- `feedback` — how much of each echo carries into the next, `0..1` (the decay).
  `0` = a single repeat; higher = more repeats before fading. Default `0.4`;
  clamped below `1` so it can't run away.
- `mix` — wet level: how loud the echoes are relative to the dry signal.
  Default `0.5`.

```lua
note(440, 0.3, 3, 1)   -- a short blip on channel 3
echo(3, 0.25, 0.5)     -- 250ms echoes, half carrying over each repeat

-- a long, spacious slap-back that rings out
echo(5, 0.4, 0.7, 0.6)
sing('la', 330, 0.5, 5)

echo(3, 0)             -- turn channel 3's echo off
```

Echo layers with `fade` (fade scales the dry + echoes together) and `vox` on the
same channel. See also `fade`, `note`, `sing`, `attr`.

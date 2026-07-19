## filt

_resonant filter on a channel (a channel-level effect)_

```lua
---@type fun(channel: integer, kind?: string, cutoff?: number, q?: number, secs?: number)
function filt(channel, kind, cutoff, q, secs)
```

Runs a resonant biquad **filter** over a playback **channel's** whole output, so
every note/`sing`/`smpl` on it is shaped the same. Like `fade`/`echo`, it's a
channel-level effect, so a chord (which sounds across one channel's lanes) filters
as a whole.

- `channel` — the channel to filter (0-based, as in `note`/`fade`).
- `kind` — the filter shape (case-insensitive):
  - `'low'` (`'lp'`) — low-pass: keep frequencies below the cutoff (muffle/darken).
  - `'high'` (`'hp'`) — high-pass: keep above the cutoff (thin/brighten).
  - `'band'` (`'bp'`) — band-pass: keep a band around the cutoff.
  - `'notch'` — cut a band around the cutoff.
  - `'off'`/`nil`/anything unknown — **disable** the filter (pass-through).
- `cutoff` — corner (low/high) or center (band/notch) frequency in Hz. Default `1000`.
- `q` — resonance. `0.707` is flat (Butterworth, no peak); higher values ring/peak
  at the cutoff (the classic squelchy sound). Default `0.707`.
- `secs` — if `> 0`, **sweep** the cutoff from its current value to `cutoff` over
  this many seconds (click-free). Default `0` = set instantly.

```lua
note(220, 4, 2, 1)               -- a sustained tone on channel 2
filt(2, 'low', 400, 6)           -- muffled low-pass, resonant (q 6)

-- a sweep: open the low-pass from 200 Hz up to 4 kHz over 3 seconds
filt(2, 'low', 200, 8)           -- start low
filt(2, 'low', 4000, 8, 3)       -- sweep up to 4 kHz over 3s

filt(2, 'off')                   -- filter off
```

Chains with `echo` (filter runs first, so echoes are filtered too), `fade`, and
`vox` on the same channel. See also `echo`, `fade`, `note`, `sing`, `attr`.

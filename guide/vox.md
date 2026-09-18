## vox

_set a channel's singing voice character (breath + vibrato)_

```lua
---@type fun(channel: integer?, cfg: { breath?: number, vib?: number, hz?: number })
function vox(channel, cfg)
```

Configures the singing voice for one **channel** — every later `sing` note on
that channel inherits it, until you change it again or reload. It's channel-scoped
like `fade`, so different channels can be different singers (a breathy lead on one,
a clean choir on another). `channel` defaults to `0`. All `cfg` keys optional:

- `breath` — aspiration noise mixed into the voice, `0..1`. `0` = clean/robotic,
  `~0.15` = airy, `~0.4` = whispery. The noise runs through the same formants, so
  it sounds like breath *in* the vowel, not hiss on top.
- `vib` — vibrato depth as a pitch fraction. `0` = off, `~0.02` = subtle,
  `~0.05` = operatic wobble.
- `hz` — vibrato rate in Hz. Default `5.5` (a natural singing rate; ~4–7 is typical).

```lua
vox(0, { breath = 0.15, vib = 0.03 })    -- channel 0: warm, gently vibrato'd
sing('la la laa', { 330, 392, 440 }, nil, 0)

vox(1, { breath = 0.4 })                 -- channel 1: a whispery voice
sing('ooo', 220, 2, 1)

vox({ breath = 0 })                      -- channel 0 back to clean/robotic
```

Because a chord sounds across one channel's lanes (see `attr{lanes}`), the whole
chord shares that channel's voice character. Applies only to `sing` (the formant
voice), not `note`/`smpl`. See also `sing`, `fade`, `attr`.

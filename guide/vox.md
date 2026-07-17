## vox

_set the singing voice character (breath + vibrato)_

```lua
---@type fun(cfg: { breath?: number, vib?: number, hz?: number })
function vox(cfg)
```

Configures the voice used by subsequent `sing` calls. It's a persistent voice
"patch" — set it once, every later `sing` note inherits it, until you change it
again or reload. All keys optional:

- `breath` — aspiration noise mixed into the voice, `0..1`. `0` = clean/robotic,
  `~0.15` = airy, `~0.4` = whispery. The noise runs through the same formants, so
  it sounds like breath *in* the vowel, not hiss on top.
- `vib` — vibrato depth as a pitch fraction. `0` = off, `~0.02` = subtle,
  `~0.05` = operatic wobble.
- `hz` — vibrato rate in Hz. Default `5.5` (a natural singing rate; ~4–7 is typical).

```lua
vox { breath = 0.15, vib = 0.03 }        -- warm, gently vibrato'd voice
sing('la la laa', { 330, 392, 440 })     -- inherits the character

vox { breath = 0, vib = 0 }              -- back to the clean robotic voice
sing('ooo', 220, 2)
```

Applies only to `sing` (the formant voice), not `note`/`smpl`. See also `sing`.

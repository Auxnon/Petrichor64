## crsh

_bitcrush a channel (a channel-level effect)_

```lua
---@type fun(channel: integer, bits: number, rate?: number)
function crsh(channel, bits, rate)
```

**Bitcrushes** a playback **channel** — the classic retro/lo-fi degrade. Two
independent knobs, both of which you'd hear on an old console or sampler:

- **bit depth** (`bits`) — quantizes the signal to `2^bits` amplitude steps,
  adding gritty quantization noise. `8` is 8-bit-console crunch, `4` is harsh,
  `1` is a nearly square-wave destruction. `16` = no quantizing (rate only).
  **`bits <= 0` disables** the whole effect.
- **sample rate** (`rate`) — latches the signal at a lower rate in Hz and holds
  it, adding the "aliasing sizzle" of cheap early samplers. `8000` is lo-fi,
  `4000` is very crunchy. Default `0` = no decimation.

Like `fade`/`echo`/`filt`/`verb` it's a channel effect, so it degrades everything
sounding on that channel (a chord crushes as a whole).

```lua
note(220, 2, 2, 1)
crsh(2, 8)              -- 8-bit crunch
crsh(2, 4, 8000)        -- 4-bit AND downsampled: full arcade destruction

sing('la', 330, 1, 3)
crsh(3, 6, 11025)       -- a lo-fi robot voice

crsh(2, 0)              -- off
```

Allocation-free, so it's cheap to punch in and out per frame:

```lua
if key('z', true) then crsh(2, 4, 6000) end   -- press: crush
if key('z') == false then crsh(2, 0) end      -- release: clean
```

Runs early in the channel chain (**grit → crsh → filt → echo → verb → fade**), so
`filt` can tame the aliasing it creates. See also `grit`, `filt`, `echo`, `verb`.

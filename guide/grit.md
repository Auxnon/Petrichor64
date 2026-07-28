## grit

_drive / distort a channel (a channel-level effect)_

```lua
---@type fun(channel: integer, amount: number, mode?: string)
function grit(channel, amount, mode)
```

**Drives** a playback **channel** into a nonlinear curve so it saturates and grows
harmonics — from a little warmth to full arcade fuzz. Like `fade`/`echo`/`filt`/
`verb` it's a channel effect, so everything on that channel distorts together
(and, as with a real amp, a chord drives harder than a single note).

- `amount` — drive knob, `0..1`. **`amount <= 0` disables** it. Low values add
  warmth/body; high values fuzz out.
- `mode` — the curve (case-insensitive):
  - `'soft'` (default) — smooth `tanh` saturation: warm, tube-ish. Level-
    compensated, so raising `amount` adds grit rather than just volume.
  - `'hard'` (`'clip'`) — hard clipping: buzzy and aggressive, squares off the peaks.
  - `'fold'` (`'foldback'`) — peaks reflect back down instead of clipping, for
    chaotic, ring-mod-flavoured harmonics. Great for broken-machine sounds.

```lua
note(110, 2, 2, 1)
grit(2, 0.3)             -- warm, driven
grit(2, 0.8, 'hard')     -- buzzy arcade fuzz
grit(2, 0.6, 'fold')     -- chaotic, metallic

grit(2, 0)               -- clean
```

Allocation-free, so it's cheap to punch in and out per frame. Pairs especially
well with `crsh` (drive first, then crush) and a `filt` after to tame the fizz:

```lua
grit(2, 0.7, 'hard')
crsh(2, 6, 8000)
filt(2, 'low', 3000)
```

Runs first in the channel chain (**grit → crsh → filt → echo → verb → fade**).
The master bus soft-limits afterwards, so heavy drive compresses rather than
clipping out. See also `crsh`, `filt`, `echo`, `verb`.

## chip

_get or set the graphics chip preset_

```lua
---@type fun(code?: string): string?
function chip(code)
```

Selects a render-pipeline preset that changes how the 3D scene is actually drawn
— not just a screen filter. Trademark-safe nods to real hardware lineage, not
console names:

- `"r00"` — modern (default). Full precision, perspective-correct texturing,
  today's filtering. A no-op if you never call `chip()`.
- `"r43"` — N64-ish, named for the N64's MIPS **VR4300**/R4300i CPU. Soft,
  bilinear-blurred, lower internal resolution, a touch of fog — the RCP's
  characteristic blur, not a geometry change.
- `"r30"` — PS1-ish, named for the PS1's MIPS **R3000A**. Real vertex wobble
  (no subpixel precision), affine texture warping, nearest-neighbor filtering,
  and ordered dithering — the actual hardware quirks, not a filter over them.

Called with no argument, returns the current preset as a lowercase string.

Pairs with `mon()`, which controls the *display* (CRT/LCD look) independently of
the chip. Any individual `attr{}` field (`resolution`, `fog`, etc.) can still be
hand-tuned after picking a chip — the preset just sets sane starting values.

Resets to `"r00"` on every app load, same as every other screen effect.

```lua
chip('r30')  -- PS1-ish wobble/affine/dither
chip('r43')  -- N64-ish blur
chip('r00')  -- back to modern
chip()       -- query, e.g. "r30"
```

A script can also request its chip on load via a first-line header comment —
see `guide/mon.md` for the combined `chip`+`monitor` header byte format.

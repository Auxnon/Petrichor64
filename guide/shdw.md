## shdw

_enable, disable or tune the shadow map_

```lua
---@type fun(on?: boolean | {on?: boolean, size?: integer, range?: number})
function shdw(on)
```

Turns the shadow pass on or off. Off by default — a script has to opt in, so
every existing app renders exactly as it did before this existed.

The shadow is cast by whichever light [lum](#lum) is currently set to: `"sun"`
and `"cone"` both cast real shadows; `"sphere"` illuminates the scene but casts
no shadow (a point light would need a cube map to shadow correctly in every
direction, which this engine doesn't do). Shadows block the light's direct
contribution only, so shadowed surfaces fall to the `ambient` level, not black.

Pass a table instead of a boolean to tune the map. `on` defaults to `true`,
and any field left out keeps its current value:

- `size` — the shadow map's edge length in texels (default `128`, clamped to
  `16..4096` and to what the GPU supports). The small default reads as soft,
  blocky shadows; raise it for crisp ones. Memory is `size² × 4` bytes, so
  `1024` is 4 MB and `4096` is 64 MB.
- `range` — sun only: how far the shadowed area reaches, in tiles (default
  `25`). It's a square of this half-width sitting just ahead of the camera.
  A smaller `range` spreads the same texels over less ground, so it sharpens
  shadows as much as a bigger `size` does, at no cost, but anything outside
  it casts and receives no shadow. A cone light's coverage comes from its
  own `lum` `range` and `angle` instead.

Every app load resets all of this to the defaults.

```lua
lum { dir = { -0.3, -0.6, -0.7 }, color = { 1, 0.95, 0.85 }, ambient = 0.3 }
shdw()                           -- shadows on, soft defaults
shdw { size = 1024, range = 12 } -- on, crisp, over a 24-tile square
shdw { on = false }              -- off, keeping size/range for later
shdw(false)                      -- off
```

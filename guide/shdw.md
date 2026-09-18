## shdw

_enable/disable the shadow map_

```lua
---@type fun(on?: boolean)
function shdw(on)
```

Turns the shadow pass on or off. Off by default — a script has to opt in, so
every existing app renders exactly as it did before this existed.

The shadow is cast by whichever light [lum](#lum) is currently set to: `"sun"`
and `"cone"` both cast real shadows (a small, deliberately low-resolution
depth pass — blocky by design, not a bug); `"sphere"` illuminates the scene
but casts no shadow (a point light would need a cube map to shadow correctly
in every direction, which this engine doesn't do).

```lua
lum { dir = { -0.3, -0.6, -0.7 }, color = { 1, 0.95, 0.85 }, ambient = 0.3 }
shdw()       -- shadows on
shdw(false)  -- shadows off
```

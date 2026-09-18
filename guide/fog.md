## fog

_distance fog (L2 retro lighting)_

```lua
---@type fun({color?: number[], dist?: number})
function fog(params)
```

Blends geometry toward a fog colour as it recedes from the camera — the classic
PS1/N64 way to hide the far clip and add depth.

- `color` — fog rgb, `{r, g, b}` in 0..1 (often the sky/horizon colour).
- `dist` — the far distance (world units) at which geometry is fully fogged.
  `0` disables fog (the default).

Fog is a flat forward-pass blend: `mix(shaded, color, clamp(dist_to_cam / dist))`.

```lua
sky:fill('9bd') -- pale sky
fog { color = { 0.6, 0.74, 0.87 }, dist = 240 } -- fade to the sky by 240 units
fog { dist = 0 } -- off
```

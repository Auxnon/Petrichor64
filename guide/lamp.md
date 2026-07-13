## lamp

_set the directional sun (L0 retro lighting)_

```lua
---@type fun({dir?: number[], color?: number[], ambient?: number})
function lamp(params)
```

Sets a single directional light for the 3D pass. Pass any subset:

- `dir` — the direction the light travels, as `{x, y, z}` (need not be
  normalised). Up is `+z`, so `{0, 0, -1}` is a light shining straight down.
- `color` — the sun's rgb intensity, `{r, g, b}` in 0..1.
- `ambient` — flat fill light, 0..1, applied everywhere.

The final shade is `ambient + max(dot(normal, -dir), 0) * color`. The engine
defaults to fullbright (`color = {0,0,0}`, `ambient = 1`), so a scene is
unlit/unchanged until you call `lamp`.

```lua
-- soft overhead sun with gentle fill
light { dir = { -0.3, -0.5, -0.8 }, color = { 0.8, 0.8, 0.75 }, ambient = 0.35 }

light { ambient = 1 } -- back to fullbright
```

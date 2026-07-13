## lamp

_set the directional sun (L0 retro lighting)_

```lua
---@type fun({dir?: number[], color?: number[], ambient?: number, sky?: number[], ground?: number[]})
function lamp(params)
```

Sets a single directional light for the 3D pass. Pass any subset:

- `dir` — the direction the light travels, as `{x, y, z}` (need not be
  normalised). Up is `+z`, so `{0, 0, -1}` is a light shining straight down.
- `color` — the sun's rgb intensity, `{r, g, b}` in 0..1.
- `ambient` — flat fill light, 0..1, applied everywhere.
- `sky` / `ground` — hemisphere ambient rgb. When given, ambient becomes
  directional: `mix(ground, sky, up)` by surface normal.z — top faces get `sky`,
  underside gets `ground`. Overrides the flat `ambient` scalar.

The final shade is `ambient + max(dot(normal, -dir), 0) * color`. The engine
defaults to fullbright (`color = {0,0,0}`, `ambient = 1`), so a scene is
unlit/unchanged until you call `lamp`.

```lua
-- soft overhead sun with gentle fill
lamp { dir = { -0.3, -0.5, -0.8 }, color = { 0.8, 0.8, 0.75 }, ambient = 0.35 }

-- hemisphere ambient: cool sky light above, warm bounce below
lamp { color = { 0.7, 0.65, 0.5 }, sky = { 0.4, 0.5, 0.7 }, ground = { 0.25, 0.2, 0.15 } }

lamp { ambient = 1 } -- back to flat fullbright
```

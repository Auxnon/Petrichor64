## lum

_set the one light_

```lua
---@type fun({shape?: string, dir?: number[], pos?: number[], color?: number[], ambient?: number, sky?: number[], ground?: number[], range?: number, angle?: number})
function lum(params)
```

Sets the engine's one light — the fantasy console has exactly one, but it can
be any of three shapes. `shape` defaults to `"sun"`, so calls that only ever
set `dir`/`color`/`ambient`/`sky`/`ground` behave exactly as they always have.

- `"sun"` — directional, infinite distance (the default). `dir` — the
  direction the light travels, as `{x, y, z}` (need not be normalised); up is
  `+z`, so `{0, 0, -1}` shines straight down. `sky`/`ground` (hemisphere
  ambient) only apply to this shape — a positioned light has no "up/down" of
  its own to hang that on.
- `"cone"` — a spotlight: `pos` (world xyz), `dir` (aim direction), `range`
  (falloff distance), `angle` (half-angle in radians — how wide the cone is).
- `"sphere"` — a point light: `pos`, `range`. No `dir`/`angle`.

`color` (rgb 0..1) and `ambient` (flat fill, 0..1) apply to every shape. The
final shade is `ambient + max(dot(normal, -L), 0) * color * atten`, where `L`
is the light direction (`dir` for sun, or the direction to `pos` for cone/
sphere) and `atten` is 1 for sun, or falls off with distance (and, for cone,
with angle from `dir`) for cone/sphere.

Only `"sun"` and `"cone"` cast shadows (see [shdw](#shdw)) — a `"sphere"`
light needs a cube map to shadow correctly in every direction, which this
engine doesn't do; it still illuminates the scene, just without shadows.

The engine defaults to fullbright (`color = {0,0,0}`, `ambient = 1`, shape
`"sun"`), so a scene is unlit/unchanged until you call `lum`.

```lua
-- soft overhead sun with gentle fill
lum { dir = { -0.3, -0.5, -0.8 }, color = { 0.8, 0.8, 0.75 }, ambient = 0.35 }

-- hemisphere ambient: cool sky light above, warm bounce below (sun only)
lum { color = { 0.7, 0.65, 0.5 }, sky = { 0.4, 0.5, 0.7 }, ground = { 0.25, 0.2, 0.15 } }

-- a torch: warm point light, short range
lum { shape = "sphere", pos = { 4, 4, 2 }, color = { 1, 0.6, 0.3 }, range = 12 }

-- a spotlight aimed down at a stage
lum { shape = "cone", pos = { 0, 0, 10 }, dir = { 0, 0, -1 }, color = { 1, 1, 1 }, range = 20, angle = 0.5 }

lum { shape = "sun", ambient = 1 } -- back to flat fullbright
```

## app.pick

_what's under the pointer, down to the texel_

```lua
---@type fun(own?:boolean, x?:number, y?:number):table|nil
function app.pick(own, x, y)
```

Ray-casts the app's scene through the camera it was last drawn with and returns
the nearest thing under the pointer (an entity standing on or set into a tile
wins over the tile at the same depth) (or under screen point `x, y`, each 0..1
from the top left), or `nil`. It tests what is actually drawn: model triangles
through each entity's current transform, billboard sprites as the camera-facing
quad they render as, and the app's tiles. Fully transparent texels are skipped,
as they are when drawn, so a click goes through the empty corners of a sprite.

With `own` set it picks this overlay's own entities instead — how a 3D panel
works out which of its buttons the pointer is over.

| field | |
|---|---|
| `kind` | `"ent"` or `"tile"` |
| `id`, `asset` | the entity's id and asset (entities) |
| `cx`, `cy`, `cz` | the tile cell (tiles) |
| `x0`, `y0`, `z0`, `x1`, `y1`, `z1` | the thing's world box: the tile's cell, or the entity's model bounds through its transform (a sprite's quad) |
| `x`, `y`, `z`, `dist` | hit point and distance, in world units |
| `nx`, `ny`, `nz` | surface normal, facing the camera |
| `tex` | the texture name drawn there, if any |
| `u`, `v`, `tw`, `th` | the texel within it, and the texture's size |
| `color` | that texel's colour, `rrggbbaa` |

Paint the texel by editing the texture with `gimg` / `tex`; the change shows on
every mesh using it.

Pass the result to [app.mesh](#mesh) for the thing's triangles, or
[app.hide](#hide) to hide it.

Only exists inside an overlay.

```lua
local h = app.pick()
if h ~= nil and h.tex ~= nil then
	local im = gimg(h.tex)
	im:pixel(h.u, h.v, "f00")
	tex(h.tex, im)
end
```

## app.mesh

_the mesh of a tile or entity, to edit_

```lua
---@type fun(hit:table):table|nil
function app.mesh(hit)
```

Takes what [app.pick](#pick) returned and hands back the thing's triangles in
its own model space, with the texture each one shows: what the model editor
(Ctrl+M) opens when you click something.

| field | |
|---|---|
| `kind` | `"model"`, `"cube"` (a tile that's only a texture) or `"sprite"` |
| `asset` | the entity's asset, or the tile's |
| `source` | where the mesh came from: `"file"`, `"code"` (`mod()`), `"texture"` (cube or sprite), `"packed"` (a file in a packed game), `"engine"` (the built-in cube) |
| `path` | for `"file"`: the glb/gltf, relative to the app |
| `p` | triangle corners, three per triangle, unshared |
| `u` | a uv per corner, in its texture's own 0..1 |
| `t` | a texture name per triangle (`""` where none could be found) |
| `x`, `y`, `z`, `rz` | where model space sits: world = (x, y, z) + the point turned `rz` radians about z |
| `scale` | the entity's scale (not applied to `p`) |
| `err` | instead of the rest, why there's no mesh |

- A model loaded from a glb/gltf is read back from that file, so the numbers
  are the file's own, not the engine's packed copy. Its texture is the one the
  file brought, named after it.
- Any other model comes out of the engine's packed copy (positions to a
  sixteenth of a unit). Each triangle's texture is found from its uvs, so a
  `mod()` cube textured six ways comes back with all six.
- A tile drawn from a texture is the textured cube it shows; a tile's model
  space starts at its cell's low corner, turned a quarter turn per rotation
  step about the cell's centre.
- A sprite is its quad stood upright, one unit across, facing -y.

Only exists inside an overlay.

```lua
local h = app.pick()
if h ~= nil then
	local m = app.mesh(h)
	if m ~= nil and m.err == nil then
		cout(m.kind .. " " .. m.asset .. ": " .. #m.t .. " triangles")
	end
end
```

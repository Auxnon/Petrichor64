## glb

_glTF model file_

```lua
---@alias glb_data {v:number[3][], i?:integer[], n?:number[3][], u?:number[2][], t?:string, up?:string}
---@type fun(path:string, data?:glb_data):boolean|{v:number[3][], i:integer[], n:number[3][], u:number[2][]}|nil
function glb(path, data)
```

Write a mesh out as a glTF file, or read one back as tables. The tables are the
same shape [mod](#mod)'s standard form takes, so the mesh you show with `mod()` is
the mesh you save with `glb()`.

**Writing** — pass a table with a `v` field (or `p` + `f`). Returns whether the
file landed.

- `v` vertex positions, `i` 0-based triangle indices (three per triangle; leave it
  out and every three verts make a triangle), `n` normals, `u` UVs. Missing normals
  are computed flat from the triangles; missing UVs are zero.
- `t` the name of a texture to embed, e.g. one made with [tex](#tex). It's saved as
  a PNG with nearest filtering, and `u` is in that texture's own 0–1 space.
- Instead of `v`/`i`, the polygon form from [mod](#mod) works too: `p` points, `f`
  faces of 1-based indices, `fu` one uv per face. Each face is exported flat with
  its own normal.
- With the polygon form's `ft` (a texture per face) and `fc` (a uv per corner),
  every texture the faces use is baked side by side into the one image the file
  carries, and the uvs moved onto it. The engine loads a glb's image as a
  texture named after the file, so naming a file after a texture you use
  elsewhere replaces that texture.
- `up="y"` rotates the file to glTF's usual Y-up for tools like Blender. Leave it
  out to keep the engine's Z-up, which is what the engine itself loads.

The extension picks the format: `.gltf` writes JSON with everything embedded,
anything else writes binary `.glb`. Paths are relative to the app's folder and
can't leave it; missing folders are created. A `.glb` saved into `assets/` loads
as a model the next time the app starts, named after the file.

```lua
mod("card", {
    v = { { -0.8, -1, 0 }, { 0.8, -1, 0 }, { 0.8, 1, 0 }, { -0.8, 1, 0 } },
    i = { 0, 1, 2, 0, 2, 3 },
    u = { { 0, 0 }, { 1, 0 }, { 1, 1 }, { 0, 1 } },
})
glb("assets/card.glb", {
    v = { { -0.8, -1, 0 }, { 0.8, -1, 0 }, { 0.8, 1, 0 }, { -0.8, 1, 0 } },
    i = { 0, 1, 2, 0, 2, 3 },
    u = { { 0, 0 }, { 1, 0 }, { 1, 1 }, { 0, 1 } },
    t = "example",
})
```

**Reading** — pass just the path (or a table without `v`, for `up="y"`). Returns
`{v, i, n, u}` with every triangle mesh in the file merged into one, or `nil` if it
can't be read. Node transforms are ignored, as they are when the engine loads a
model.

```lua
local m = glb("assets/card.glb")
if m then mod("card2", m) end
```

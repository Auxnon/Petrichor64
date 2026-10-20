## unwrap

_lay a mesh flat onto an image_

```lua
---@type fun(mesh:{v:number[3][], i:integer[]?, n:number[3][]?}, opts:{w:integer?, h:integer?, pad:integer?, mode:string?}?):{v:number[3][], n:number[3][]?, u:number[2][], i:integer[], from:integer[], charts:integer}
function unwrap(mesh, opts)
```

Works out uvs for a mesh, for a `w` x `h` image (default 64 x 64), with `pad`
texels between pieces (default 1). Pass the result straight to `mod()` with a
texture of that size:

```lua
local u = unwrap({v = verts, i = inds}, {w = 32, h = 32})
mod("crate", {v = u.v, n = u.n, u = u.u, i = u.i, t = {"crate_skin"}})
```

Connected triangles facing the same way become one chart, projected flat and
oriented as you'd see it from that side. Every chart is drawn at the same scale,
so texels are the same size all over the model, and starts on a whole texel.

- `mode = "box"` (default): six directions, ±x ±y ±z. Suits boxy meshes.
- `mode = "flat"`: everything projected along the mesh's thinnest axis, one
  chart. Suits signs, panels and leaves.

Vertices on a chart's edge are split, so the result has its own vertex list:
`from[k]` is the input vertex (0-based, like `i`) that vertex `k` came from,
for carrying anything else per vertex across. `n` is carried if given.

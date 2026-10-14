## mod

_set model_

```lua
---@alias regular_data {v:number[3][], u?:number[2][]}, i?:integer[], n?:number[3][], t?:string|string[]}
---@alias quad_data {q:number[3][], u?:number[2][], n?:number[3][], t?:string|string[]}
---@alias cube_data {t:string|string[]}
---@alias polygon_data {p:(number[3]|vec3)[], f:integer[][], fu?:number[2][], t?:string, scale?:number}
---@alias line_data {l:(number[3]|vec3)[], lw?:number|number[], lu?:number[2][], t?:string, scale?:number}
---@type fun(asset:string, data:regular_data|quad_data|cube_data|polygon_data|line_data)
function mod(asset,data)
```

Create a mesh based on provided data and store as a model asset. A mesh can be created under 3 distinct types: Either fully by vertices and indices etc, as quad data with the rest interpreted via an experimental mechanism, or as a simple cube with more granular control the normal cube map.

**The standard method** passes in vertices (v) indices (i) and UVs (u). Normals(n) can be used as well but currently not rendered. Notice usage of indices can reduce redundant vertices being made. If indices are not used the system will assume the verts are being staggered by 3 and will wastefully create a mesh from that. The following creates a "card" shape that's 1.6 wide on the x axis and 2 long on the y axis with origin centered.

```lua
mod("card", {
    v = { { -0.8, -1, 0 }, { 0.8, -1, 0 }, { 0.8, 1, 0 }, { -0.8, 1, 0 } },
    i = { 0, 1, 2, 0, 2, 3 },
    u = { { 0, 0 }, { 1, 0 }, { 1, 1 }, { 0, 1 } }
    })
```

**The quad method** is done by passing a data with a q field set and is considerably simpler if not less predictable. A quad is 4 vertices, listed in order around its edge (1→2→3→4, no crossing), so by passing in 8 vertices we create 2 quad faces. Indices are automatic. UVs are estimated based on right hand rule, aligning the top edge to the 1st and 2nd vertices. The UV is uniformly applied and always fit rather then clipped. Estimations may not be perfect. Expect bugs. The mechanism may change in the future. The following creates a plane that's 1 wide on the X axis, and 1 tall on the Z axis.

````lua
`t` is a texture name, or a list of names that the faces cycle through; a bare string is the same as a one-element list (`t="tex"` == `t={"tex"}`). `t` is optional in every form. A model built without one gets the engine's fallback
texture — a grey checker that lives in the first slot of the atlas and is always
present — so a mesh can be built first and textured later through the entity. An app
that ships its own image named `default` replaces it.

```lua
mod("flat", {q={{0,0,0}, {1,0,0},{1,0,1},{0,0,1}}})
````

**The polygon method** takes a mesh the way a modelling tool keeps one: a list of
points `p` and a list of faces `f`, each face a list of **1-based** indices into `p`
(Lua's own convention — note `i` above is 0-based), wound so the right-hand rule
points out of the face. Faces can have any number of corners; the engine fans
each one from its first corner (exact for convex faces), gives it a flat normal,
and gives each face its own copy of its corners. `fu` is one UV per face — point
it at a texel of `t` to colour faces from a palette. Points can be `{x, y, z}`
tables or `vec3` values. All of the triangle work happens in the engine, so
rebuilding a model this way every frame is cheap.

```lua
mod("wedge", {
    p = { vec3(0, 0, 0), vec3(1, 0, 0), vec3(1, 1, 0), vec3(0, 1, 0), vec3(0, 0, 1), vec3(0, 1, 1) },
    f = { { 1, 4, 3, 2 }, { 1, 2, 5 }, { 4, 6, 3 }, { 1, 5, 6, 4 }, { 2, 3, 6, 5 } },
})
```

**The line method** builds square beams along line segments: `l` is a list of
endpoints taken in pairs, `lw` the beam's thickness (one number, or one per
segment), `lu` one UV per segment. Beams look the same from every angle, which a
flat quad doesn't — for wireframes, grids, gizmos and debug lines.

```lua
mod("axes", {
    l = { { 0, 0, 0 }, { 1, 0, 0 }, { 0, 0, 0 }, { 0, 0, 1 } },
    lw = 0.02,
})
```

`scale` (polygon and line methods) multiplies every position before the model is
packed. Model vertices are stored at 1/16 of a unit, so a fine mesh — a thin grid
line, a small handle — loses its shape; build it `scale` times larger and show it
on an entity with `ent.scale = 1 / scale` to keep 1/(16 × scale) precision.

[glb](#glb) accepts the same polygon tables, so a mesh shown with `mod()` can be
saved as a glTF file unchanged.

**The cube method** is the simplest of all as it's literally just re-texturing a cube by passing in texture assets directly. the texture array can be 6 in length for each face in order of Z+, X+, Y+, X-, Y-, Z-. Any omitted asset will default to the first

```lua
mod("block",{"top","right","front","left","back","bottom"})
```

### mod

_set model_

```lua
---@alias regular_data {v:number[3][], u?:number[2][]}, i?:integer[], n?:number[3][], t:string[]}
---@alias quad_data {q:number, u?:number[2][], n?:number[3][],t:string[]}
---@alias cube_data {t:string[]}
---@type fun(asset:string, data:regular_data|quad_data|cube_data)
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

**The quad method** is done by passing a data with a q field set and is considerably simpler if not less predictable. A quad is 4 vertices, so by passing in 8 vertices we create 8 quad faces. Indices are automatic. UVs are estimated based on right hand rule, aligning the top edge to the 1st and 2nd vertices. The UV is uniformly applied and always fit rather then clipped. Estimations may not be perfect. Expect bugs. The mechanism may change in the future. The following creates a plane that's 1 wide on the X axis, and 1 tall on the Z axis.

````lua
```lua
mod("flat", {q={{0,0,0}, {1,0,0},{1,0,1},{0,0,1}}})
````

**The cube method** is the simplest of all as it's literally just re-texturing a cube by passing in texture assets directly. the texture array can be 6 in length for each face in order of Z+, X+, Y+, X-, Y-, Z-. Any omitted asset will default to the first

```lua
mod("block",{"top","right","front","left","back","bottom"})
```

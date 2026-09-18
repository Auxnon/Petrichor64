## hit (collision system)

The collision system detects overlap between entities, and between an entity
and the solid world tiles around it, and reports a **normal** and **depth**
for each hit so a script can decide what to do about it — push apart, slide,
stop, whatever the game needs. Rust computes the geometry; Lua composes the
response.

Four entry points, all under the `hit` table (`hit.all()`, `hit.cell(id)`,
`hit.grid(id)`, `hit.pair(a, b)` — see their own doc pages), plus three fields
on every [entity](#entity): `hit_shape`, `hit_size`, `hit_offset`.

**`hit.pair(a, b)` is a function, not a colon method** (`a:hit(b)`, as first
asked for) — silt-lua's native-call return plumbing isn't reachable from
outside the interpreter crate, so a `LuaEnt` userdata method has no way to
bridge to a channel-backed native and get a return value back; only fire-and-
forget calls work from there (the same trick `entity:copy()` uses to reach
`make()`). A plain function has normal engine access, so that's the shape.

### Shapes and defaults

Every collider is one of two axis-aligned shapes — no rotation is modeled,
matching a simple "AABB + swept AABB" collision system:

- **Cylinder** — Z-axis-aligned: an XY radius plus a Z range. The default for
  a billboard sprite (an entity whose `asset` doesn't resolve to a real
  model), since there's no mesh to derive a box from.
- **Box** — axis-aligned half-extents. The default for a real model entity,
  baked from the model's own vertex bounds at `mod()`/load time (so it costs
  nothing at collision time) and scaled by the entity's current `scale`/
  `size` — a bigger entity gets a bigger box automatically.

Set `hit_shape`/`hit_size`/`hit_offset` on an entity to override either
default — see [entity](#entity).

### Reading a hit

Every hit is `{normal = {x, y, z}, depth = n}` (plus `a`/`b` ids for
`hit.all()`, or a `tile` coordinate for `hit.cell()`). `normal` is the unit
direction to move the entity away from what it hit; `depth` is how far it's
currently overlapping along that direction. The simplest response is to push
straight out:

```lua
ent.x = ent.x + h.normal[1] * h.depth
ent.y = ent.y + h.normal[2] * h.depth
ent.z = ent.z + h.normal[3] * h.depth
```

A wall-slide (move along the surface instead of stopping dead) is one line
with vectors, using `ent.vx/vy/vz` as the entity's own velocity convention:

```lua
local vel = vec3(ent.vx, ent.vy, ent.vz)
local n = vec3(h.normal[1], h.normal[2], h.normal[3])
local slide = vel - n * vel:dot(n) -- the tangential component
```

### Performance notes

- **`hit.all()`** is the O(n²)-avoided batch query: entities are bucketed by
  which tile-chunk (roughly a 512-unit cube) their collider center falls in,
  and only same-bucket pairs are tested. Two colliders that overlap but
  straddle a chunk boundary line can be missed — an accepted v1 simplification
  given how large a chunk is relative to anything likely to collide.
- **`hit.cell()`** reads the world grid through a locally-held mirror that's
  synced once a frame, not a per-tile round trip — cheap enough to call every
  frame for a moving entity. That also means a tile placed this frame (via
  `tile()`) won't be visible to `hit.cell()` until next frame.
- **`hit.grid()`** — the moving/rotating counterpart to `hit.cell()`, testing
  against another entity's own tile grid (`ent:tile(...)`, see
  [entity](#entity)) instead of the static world. A grid can be rotated, so
  this can't just reuse `tile_box`/AABB tests directly against world space:
  the query collider is moved into the grid's local space (by the owning
  entity's inverse transform) and tested there, then the resulting normal is
  rotated back — a 3-tier cascade (grid bounds → chunk bounds → tile) that
  runs as one Rust call, never exposed to Lua as separate steps. Exact for a
  cylinder query; an approximation for a box query under a non-90°-multiple
  rotation (its edges no longer line up with the grid's local axes once
  transformed) — same first-pass-fidelity caveat as the rest of this system.

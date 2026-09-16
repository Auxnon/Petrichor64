## hit.grid

_every hit between an entity's collider and another entity's tile grid_

```lua
---@type fun(id: integer): table[]
function hit.grid(id)
```

The ent-vs-entity-grid collision query — tests entity `id`'s collider against
every *other* entity's own tile grid (`ent:tile(...)`, see `guide/entity.md`),
not the static world grid (`hit.cell` is the world-grid equivalent). An
entity's grid never tests against itself. Returns an array of
`{owner, tile, normal, depth}`:

- `owner` — the id of the entity that owns the grid this tile belongs to.
- `tile` — `{x, y, z}`, the tile's integer coordinates in the *owner's own*
  grid-local space (not world space — the owner may be moved/rotated).
- `normal` — `{x, y, z}`, the unit direction (already rotated into world
  space) to move `id`'s entity out of that tile.
- `depth` — how far the collider currently overlaps that tile, along `normal`.

Internally this runs a 3-tier cascade entirely in Rust (owner's overall grid
bounds → chunk bounds → individual tile) so a mostly-empty moving platform
stays cheap to query every frame — none of the tiers are exposed to Lua
individually. See `guide/hit.md` for the full collision system and
`guide/entity.md` for grid-owning entities.

```lua
for _, h in ipairs(hit.grid(player.id)) do
  player.x = player.x + h.normal[1] * h.depth
  player.y = player.y + h.normal[2] * h.depth
  player.z = player.z + h.normal[3] * h.depth
end
```

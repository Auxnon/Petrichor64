## hit.cell

_every solid tile an entity's collider currently overlaps_

```lua
---@type fun(id: integer): table[]
function hit.cell(id)
```

The ent-vs-tile collision query — reads the world grid directly (no per-tile
round trip), so it's cheap to call every frame for a moving entity. Returns an
array of `{tile, normal, depth}`:

- `tile` — `{x, y, z}`, the solid tile's integer grid coordinates.
- `normal` — `{x, y, z}`, the unit direction to move the entity out of that
  tile (push it by `normal * depth`).
- `depth` — how far the collider currently overlaps that tile, along `normal`.

See `guide/hit.md` for the full collision system.

```lua
for _, h in ipairs(hit.cell(player.id)) do
  player.x = player.x + h.normal[1] * h.depth
  player.y = player.y + h.normal[2] * h.depth
  player.z = player.z + h.normal[3] * h.depth
end
```

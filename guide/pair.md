## hit.pair

_pairwise collision test between two entities_

```lua
---@type fun(a: entity, b: entity): table?
function hit.pair(a, b)
```

A function, not a colon method (`a:hit(b)`, as `hit.md` explains why) — pass
both entities directly. Returns `nil` if they don't overlap, or:

- `normal` — `{x, y, z}`, the unit direction to move `a` to separate it from
  `b` (push `a` by `normal * depth` to just clear the overlap).
- `depth` — how far the two colliders currently overlap, along `normal`.

Works across bundles (the two entities don't need to belong to the caller's
own bundle) since both are passed by value. See `guide/hit.md` for the full
collision system, and `hit.all()` for the batch ent-vs-ent query.

```lua
local h = hit.pair(player, enemy)
if h then
  player.x = player.x + h.normal[1] * h.depth
  player.y = player.y + h.normal[2] * h.depth
end
```

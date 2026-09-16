## hit.all

_every currently-overlapping ent-vs-ent pair_

```lua
---@type fun(): table[]
function hit.all()
```

The batch collision query, computed entirely in Rust — the O(n²) pairwise loop
never runs in Lua. Returns an array of `{a, b, normal, depth}`:

- `a`, `b` — the two entities' ids.
- `normal` — `{x, y, z}`, the unit direction to move `a` to separate it from
  `b` (push `a` by `normal * depth` to just clear the overlap).
- `depth` — how far the two colliders currently overlap, along `normal`.

Only entities in the same "chunk bucket" (roughly a 512-unit cube) are tested
against each other — see `guide/hit.md` for the full collision system.

```lua
for _, h in ipairs(hit.all()) do
  cout(h.a .. " hit " .. h.b .. " depth " .. h.depth)
end
```

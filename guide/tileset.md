## tileset

_mesh tiles together_

```lua
---@type fun(name:string, tiles:string[]|nil, opts?:{look?:'block'|'smooth', pieces?:table<string|integer, string>})
function tileset(name, tiles, opts) end
```

Tiles in a tileset are drawn together on the _dual grid_ instead of as one cube
each: every 2×2×2 group of neighbouring cells becomes one _piece_, picked by
which of its 8 cells are solid (the _case_). The tiles themselves don't change —
`istile`, `gtile`, rays and collision still see cubes.

- `tiles` — the tile textures that belong to the set. A tile in two sets belongs
  to the later one. `nil` or `{}` removes the set.
- `look` — what to draw for a case with no piece of your own:
  - `'block'` (default): the same cubes as plain tiles, without the faces two
    neighbours share.
  - `'smooth'`: marching cubes: flat-shaded 45° slopes and corners.
- `pieces` — your own models by case. A piece fills one cell's space (like a
  model tile), from the centre of the lowest cell (0,0,0) to the centre of the
  highest (1,1,1), and also covers its three turns about z unless that case has
  its own piece. A case is 8 characters, the corners `(x,y)` of the bottom layer
  in the order `(0,0) (1,0) (0,1) (1,1)`, then the top layer the same way;
  spaces, `_` and `/` are ignored. An integer 0..255 works too (bit
  `x + 2y + 4z`).

Pieces sit between cell centres, so a piece's middle is the corner where its
8 cells meet: a floor piece for `"1111 0000"` repeats once per tile, offset half
a tile from them. Like any model, a piece's texture must be RGBA.

Every solid tile counts toward the case, in the set or not, so a set never
leaves a hole against other tiles. Model tiles never join a set.

```lua
tileset('hills', { 'grass', 'dirt' }, { look = 'smooth' })
tileset('walls', { 'brick' }, {
  pieces = {
    ['1111 0000'] = 'wall_floor', -- the bottom half solid
    ['1100 1100'] = 'wall_side',  -- the -y half solid; turns cover the others
  },
})
tileset('hills') -- back to plain cubes
```

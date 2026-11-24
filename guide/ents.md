## app.ents

_every entity the app has_

```lua
---@type fun():table[]
function app.ents()
```

The edited app's live entities, each `{id, asset, x, y, z, rx, ry, rz,
scale}` as they stand this frame. `id` is the one [app.pick](#pick) reports
and [app.put](#put) takes.

An entity made by a script also has `file`, `line` and `col`: the script's
name (`main.lua`) and where the `make` call that created it starts, counted
from 1, in the source as it was when that call ran.

Only exists inside an overlay.

```lua
local list = app.ents()
for i = 1, #list do
	local e = list[i]
	print(e.id, e.asset, e.x, e.y, e.z)
end
```

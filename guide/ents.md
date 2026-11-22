## app.ents

_every entity the app has_

```lua
---@type fun():table[]
function app.ents()
```

The edited app's live entities, each `{id, asset, x, y, z, rx, ry, rz,
scale}` as they stand this frame. `id` is the one [app.pick](#pick) reports
and [app.put](#put) takes.

Only exists inside an overlay.

```lua
local list = app.ents()
for i = 1, #list do
	local e = list[i]
	print(e.id, e.asset, e.x, e.y, e.z)
end
```

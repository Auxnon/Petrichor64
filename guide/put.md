## app.put

_move one of the app's entities_

```lua
---@type fun(id:integer, at:table):boolean
function app.put(id, at)
```

Sets where one of the edited app's entities is: any of `x`, `y`, `z`, `rx`,
`ry`, `rz`, `scale` in `at`; the rest stay. It writes the entity itself, so
the app sees the new values, and if the app's own code keeps placing the
entity every frame its code wins on the next loop. False when there's no such
entity.

Only exists inside an overlay.

```lua
local h = app.pick()
if h ~= nil and h.kind == "ent" then
	local up = { z = h.z0 + 1 }
	app.put(h.id, up)
end
```

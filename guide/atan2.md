## atan2

_angle of a point_

```lua
---@type fun(y:number, x:number):number
function atan2(y, x)
```

The angle in radians from the +x axis to the point `(x, y)`, between `-pi` and
`pi`. Note **y comes first**, as in most languages. Uses the standard rust
library. It's what you want for turning something to face a target: on the
ground plane, 0 faces +x and `tau/4` faces +y, which matches an entity's `rz`
and the yaw in [cam](#cam). See [sin](#sin) and [cos](#cos) for the way back.

```lua
local dx = target.x - guy.x
local dy = target.y - guy.y
guy.rz = atan2(dy, dx) -- face the target
assert(atan2(1, 0) == tau / 4)
assert(atan2(0, -1) == pi)
```

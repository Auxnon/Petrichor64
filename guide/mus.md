## mus

_check mouse_

```lua
---@type fun()
function mus()
```

Returns an object representing all mouse data

- `x` - x position from 0 to 1 across screen represents far left to far right
- `y` - y position from 0 to 1 represents top to bottom edges
- `m1` - mouse button 1 pressed, usually left
- `m2` - mouse button 2 pressed, usually right
- `m3` - mouse button 3 pressed, usually middle

- `dx` - delta x represents change between ticks
- `dy` - delta y represents change between ticks
- `vx` - X field for screen space un-transformed to a ray that can be used for aligning or firing objects from camera. Bullets, picking, etc.
- `vy` - same for Y
- `vy` - same for Z
- `scroll` - scroll delta from -1 to 1

```lua
m=mus()

if m.scroll >0 then
scroll_up(m.scoll)
end

if m.m1 then
fire()
end
```

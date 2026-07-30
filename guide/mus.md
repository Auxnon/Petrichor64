## mus

_check mouse_

```lua
---@type fun()
function mus()
```

Returns an object representing all mouse data

- `x` - x position from 0 to 1 across screen represents far left to far right
- `y` - y position from 0 to 1 represents top to bottom edges
- `m1` - mouse button 1 pressed, usually left. A **boolean**, not 0/1 — compare it
  directly (`if m.m1 then`), don't write `m.m1 > 0`. On a touchscreen the primary
  finger drives `x`/`y` and contact reads as `m1`, so pointer code needs no changes
  to work on a phone.
- `m2` - mouse button 2 pressed, usually right
- `m3` - mouse button 3 pressed, usually middle

- `dx` - delta x represents change between ticks
- `dy` - delta y represents change between ticks
- `vx`, `vy`, `vz` - the cursor unprojected into the world: a **normalized direction**
  from the camera through the pointer, for aligning or firing objects from the camera
  (bullets, picking, etc). It points *away* from the camera into the scene — at screen
  centre it equals the camera's forward — so a point `d` units out along it is
  `cam + v * d`:

  ```lua
  m = mus()
  thing.x = m.vx * 12
  thing.y = m.vy * 12
  thing.z = m.vz * 12   -- 12 units out, under the cursor
  ```

  Aspect-correct, including very tall or wide windows (verified on a phone: a cube
  placed this way lands centred on the cursor).
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

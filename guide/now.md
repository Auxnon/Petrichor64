## now

_engine clock_

```lua
---@return number
function now()
```

Seconds since the engine started, as of the current frame. Frames are paced by the display, so a fixed step per `loop()` moves faster on a faster screen. Scale movement by the time between frames instead:

```lua
local last = 0
function loop()
    local t = now()
    local dt = t - last
    last = t
    x = x + speed * dt -- units per second, whatever the frame rate
end
```

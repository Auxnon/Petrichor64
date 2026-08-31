## mgrab

_grab (capture) the mouse_

```lua
---@type fun(on?: boolean)
function mgrab(on)
```

Capture the mouse for relative "look" control. While grabbed, `mus()` reports
raw movement in `dx`/`dy` (ideal for FPS-style camera rotation) and the system
cursor is hidden. Call `mgrab(false)` to release it.

On the web build the pointer is locked on the next click of the canvas (browsers
only grant pointer-lock from a user gesture); press `Esc` to release, and it
re-locks on the next click while still grabbed. The console always frees the
mouse while open.

```lua
mgrab()      -- capture
mgrab(true)  -- capture (explicit)
mgrab(false) -- release
```

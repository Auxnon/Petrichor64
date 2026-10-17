## app.view

_the app's camera and the pointer ray_

```lua
---@type fun():table|nil
function app.view()
```

The camera the scene was last drawn with — `x`, `y`, `z`, `yaw`, `pitch`, as
`cam` takes them — and the ray under the pointer: origin `ox`, `oy`, `oz` and
unit direction `dx`, `dy`, `dz`, all in world units. An overlay that never
calls `cam` draws through this camera, so this is how it places 3D panels
facing the viewer. `nil` before the first frame.

Only exists inside an overlay.

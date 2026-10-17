## app.project

_world point to screen_

```lua
---@type fun(x:number, y:number, z:number):{x:number,y:number,depth:number}|nil
function app.project(x, y, z)
```

Where a world point lands on screen (0..1 from the top left) through the app's
camera, and how far in front of it, or `nil` if it's behind the camera.

Only exists inside an overlay.

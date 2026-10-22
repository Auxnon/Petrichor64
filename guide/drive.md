## app.drive

_draw the scene through the overlay's own camera_

```lua
---@type fun(on?:boolean)
function app.drive(on)
```

From now on the scene is drawn through the camera this overlay sets with `cam`,
so it can fly around the app it edits: the Ctrl+M model editor orbits with it.
Start from `app.view()` to keep the angle the app had. `app.drive(false)` hands
the view back to the app, and closing the overlay always does, so the app's
camera is exactly where it left it. If the app sets `cam` meanwhile (it isn't
paused), that's the camera it gets back.

`mus()`, `app.view()`, `app.pick()` and `app.project()` all follow the camera
the scene is drawn with. Pair it with `attr{front=true}` to keep the overlay's
own 3D over the scene.

Only exists inside an overlay.

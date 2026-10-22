## app.png

_write a texture out as a png file_

```lua
---@type fun(texture:string, path:string):boolean
function app.png(texture, path)
```

Writes the texture's current pixels to `path` in the app being edited, as a
png, and returns whether it landed. Missing folders are created; the path
can't leave the app's folder. Save it under `assets/` and the app loads it as
a texture of that name on its next start.

The model editor uses this to ship its palette next to a model saved as code.

Only exists inside an overlay, over an app loaded from a folder.

```lua
if app.png("chor_pal", "assets/crate_pal.png") then cout("saved") end
```

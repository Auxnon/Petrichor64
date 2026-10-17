## app.source

_where a texture could be saved_

```lua
---@type fun(texture:string):{ok:boolean, path:string?, err:string?}
function app.source(texture)
```

Whether the texture can be saved back with `app.save`: `ok` with `path`, the
image file relative to the app's folder (`assets/wall.png`), or `err` saying why
not — most often `made by code`: the app drew it with `nimg` + `tex`, so
there is no file to write. A tile of a sheet (`name.16.png`) reports the sheet.

Only textures the edited app loaded from an image file inside its own folder
qualify, and only while the app hasn't redrawn them from code since.

Only exists inside an overlay.

## app.save

_write a texture back to its image file_

```lua
---@type fun(texture:string):{ok:boolean, path:string?, err:string?}
function app.save(texture)
```

Writes the texture's current pixels into the image file the edited app loaded
it from (see `app.source`), and returns `ok` with the `path` written or `err`.
A tile only replaces its own square of the sheet; the rest of the file is kept.
The file is written to a temporary name and renamed over the original, so a
failed save leaves it as it was. The app's next load picks the change up.

Only exists inside an overlay.

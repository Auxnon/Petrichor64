## app.texinfo

_a texture's size, tile and file_

```lua
---@type fun(texture:string):{w:integer, h:integer, tile:integer?, new:boolean, ok:boolean, path:string?, err:string?}|nil
function app.texinfo(texture)
```

`w` and `h` are the texture's size in pixels. `tile` is set when it is one tile
of a sheet (`name.16.png` gives `tile = 16`). `new` means `app.newtex` made it
and it hasn't been saved yet. `ok`/`path`/`err` are the same as `app.source`:
the file a save writes, relative to the app, or why there isn't one.

`nil` for a texture that doesn't exist.

Only exists inside an overlay.

## app.colors

_a texture's palette, best guess_

```lua
---@type fun(texture:string):string[]
function app.colors(texture)
```

The texture's distinct colours as `rrggbbaa` strings, most frequent first (up to
16), ignoring transparent pixels. Good for offering the colours a texture was
painted with.

Only exists inside an overlay.

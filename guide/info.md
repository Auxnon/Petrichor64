## app.info

_the edited app's folder, and whether it can be saved into_

```lua
---@type fun():{dir:string, packed:boolean}|nil
function app.info()
```

`dir` is the folder the edited app was loaded from. `packed` is true when it
came from a packed `.game.png` instead: there are no files to write, so
`app.save` and `app.source` refuse everything. The paint overlay shows this
up front instead of a SAVE button.

Only exists inside an overlay.

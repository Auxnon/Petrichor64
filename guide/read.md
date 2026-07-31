## app.read

_read a file out of the app being edited_

```lua
---@type fun(path:string):string|nil
function app.read(path)
```

Load a file from the app this overlay was opened to edit, as a UTF-8 string. Nil if
it isn't there, or if the path tries to climb out of the app's folder.

Only exists inside an overlay. The `app` table is not built for a game's own Lua, so
an app cannot reach its neighbours' files — see `overlay` in the console guide.

Paths are relative to the edited app's folder, the same shape `io.get` uses:

```lua
local src = app.read("scripts/main.lua")
```

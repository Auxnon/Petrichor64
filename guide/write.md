## app.write

_write a file into the app being edited_

```lua
---@type fun(path:string,contents:string):boolean
function app.write(path, contents)
```

Overwrite a file in the app this overlay was opened to edit. Returns whether it
landed. The file is replaced whole; there is no append.

Only exists inside an overlay, and the path cannot leave the edited app's folder.

Writing does not restart the app — call `app.reload()` when you want it picked up:

```lua
if app.write("scripts/main.lua", src) then app.reload() end
```

## io.get

_read file to string_

```lua
---@type fun(path:string):string|nil
function io.get(path)
```

Synchronously load a file as a UTF-8 string, binary files are not supported at this time.

```lua
local contents=io.get("file.lua")
```

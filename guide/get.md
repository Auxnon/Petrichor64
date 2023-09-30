## io.get

_read file t string_

```lua
---@type fun(path:string):string|nil
function io.get(path)
```

Load a file as utf8 string, binary files are not supported.

```lua
local contents=io.get("file.lua")
```

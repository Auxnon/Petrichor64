## io.set

_write string to a file_

```lua
---@type fun(path:string, buffer:string):boolean
function io.set(path, buffer)
```

Synchronously write a UTF-8 string to file relative to the game folder, binary files are not supported at this time. Returns true if successful.

```lua
if io.set("file.lua","function test() print('hiya') end") then print("success!") end
```

## io.set

_write string to a file_

```lua
---@type fun(path:string, buffer:string)
function io.set(path, buffer)
```

Load a file as utf8 string, binary files are not supported.

```lua
io.set("file.lua","function test() print('hiya') end")
```

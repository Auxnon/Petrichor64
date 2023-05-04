### dtile

_delete tile_

```lua
---@type fun(x?:integer, y?:integer, z?:integer)
function dtile( x, y, z) end
```

Crude deletion of a 16x16x16 chunk. Chunk is hardset at this size for now. Technically very efficient for large area tile changes. Not including arguments deletes all tiles within the world by and is the fastest way to clear the map.

```lua
dtile(0,0,0) -- remove chunk at origin
dtile(128) -- valid, evals to 128,0,0;
dtile() -- remove all chunks by dumping memory
```

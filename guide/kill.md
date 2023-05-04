### kill

_destroy entity_

```lua
---@type fun(ent:entity)
function kill(ent)
```

Remove an entity from the world. This must be called if an entity is dropped as their is currently no way for the renderer to know the lua instance is no longer in memory.

```lua
ent=make("example",0,0,0) -- create
kill(ent) -- remove
```

## sub

_load child process_

```lua
---@type fun(app:string)
function sub(app)
```

WIP

Operates under the same logic as the load console command. Loads another app or game while remaining in the current lua context. Currently limited to loading an app from disk. This allows overlaying of entities and tiles within the world and each acts on it's own thread. This will be used for in-engine editors in the future.

```lua
sub("test/game")
```

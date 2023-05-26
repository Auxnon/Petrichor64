## sky

_get skybox draw target_

```lua
---@type image
sky
```

The counterpart to [gui](#gui), this operates on the back or skybox raster. Must specify `sky` as the caller for draw commands or will assume `gui` was intended

```lua
sky:fill("0FF")
for i=0,20 do
    sky:img(gimg("cloud"),rnd(),rnd())
-- it's faster to set gimg to a variable but oh well
end
```

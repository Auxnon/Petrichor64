## nimg

_new image_

```lua
---@type fun(width:integer, height:integer):image
function nimg(width,height)
```

Create new image userdata of the specified size. This image is not a texture and serves no purpose other then to be an editable raster until applied

```lua
red=nimg(16,16)
red:fill("F00")
tex("red",red)
```

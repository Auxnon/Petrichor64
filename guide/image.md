### image (userdata)

An image instance ties to image raster userdata hosted inside the lua context. Simple draw commands can be used to edit and animate in real time and then rendered on to a texture via the `tex` command. True live editing of a texture is currently only possible by editing the `gui` and `sky` constants. These constants are always in global scope by default and called such as `gui:text("test")` or `sky:fill("F00")`. Additionally a new image can be created via `nimg` and then set to a texture via `tex` for similar effect.

> The image type is not to be confused with the `img` method on the image type itself, as it can be used independently as `img` which is really an alias for `gui:img`

### Image methods\*\*

### line

draw a 1 pixel thick line from point 1 to point 2 with optional [color](#color)

```lua
--- @field line fun(self:image, x:gunit, y:gunit, x2:gunit, y2:gunit, rgb?:rgb)
im:line(x,y,x2,y2,"f00")
```

### rect

draw a filled square at position x,y with dimensions width height

```lua
--- @field rect fun(self:image, x:gunit, y:gunit, w:gunit, h:gunit, rgb?:rgb)
im:rect(x,y,width,height,"000")
```

### rrect

marginally more expensive filled rectangle with the ability to round the edges

```lua
--- @field rrect fun(self:image ,x:gunit, y:gunit, w:gunit, h:gunit, ro:gunit, rgb?:rgb)
im:rrect(x,y,width,height,ro,"000")
```

### img

draw another image on this image

```lua
--- @field img fun(self:image, source_image:image, x?:gunit, y?:gunit)
im:img(source_image,x,y)
```

### text

draw text on the image. Font spaces automatically.

```lua
--- @field text fun(self:image, txt:string, x?:gunit, y?:gunit, rgb?:rgb)
im:text("message",x,y,"555")
```

### pixel

draw a single pixel directly on image. Not efficient. Working on a better mechanism.

```lua
--- @field pixel fun(self:image, x:integer, y:integer,rgb?:rgb)
im:pixel(x,y,{255,255,0})
```

### clr

clears the image

```lua
--- @field clr fun(self:image)
im:clr()
```

### fill

fill the image with a color

```lua
--- @field fill fun(self:image, rgb?:rgb)
im:fill("F0F")
```

### copy

clones image to a new image userdata instance

```lua
--- @field copy fun(self:image):image
im2=im:copy()
```

### raw

WIP currently just returns raw pixel data, no way to set

```lua
--- @field raw fun(self:image):integer[] image
data=im:raw()
```

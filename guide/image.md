### image (userdata)

```lua
--- @class image



--- @field text fun(self:image, txt:string, x?:gunit, y?:gunit, rgb?:rgb) draw text on image
--- @field img fun(self:image, im:image, x?:gunit, y?:gunit) draw another image on image
--- @field pixel fun(self:image, x:integer, y:integer,rgb?:rgb) draw pixel directly on image
--- @field clr fun(self:image) clear image
--- @field fill fun(self:image, rgb?:rgb) fill image with color
--- @field raw fun(self:image):integer[] image return raw pixel data
--- @field copy fun(self:image):image clones to new image

```

An image instance ties to image raster userdata hosted inside the lua context. Simple draw commands can be used to edit and animate in real time and then rendered on to a texture via the `tex` command. True live editing of a texture is currently only possible by editing the `gui` and `sky` constants. These constants are always in global scope by default and called such as `gui:text("test")` or `sky:fill("F00")`. Additionally a new image can be created via `nimg` and then set to a texture via `tex` for similar effect.

### Image methods\*\*

### line

draw a 1 pixel thick line from point 1 to point 2 with optional (color)[color]

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
--- @field rrect fun(self:image ,x:gunit, y:gunit, w:gunit, h:gunit,ro:gunit, rgb?:rgb)
im:rrect()


The image type, a userdata object, is not to be confused with the (img)[img] command.
```

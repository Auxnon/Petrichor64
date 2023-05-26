## gunit (type)

_graphical unit_

```lua
--- @alias gunit number | integer | string
```

The awkwardly named `gunit` type ( pronounced either G unit, gun it, or goonit depeding on how crazy you want to sound) is simply multiple ways to define the placement and size on a screen. A decimal assumes the role of a percentage while an integer is a raw pixel value. Take care to round if necessary when using dynamic parameters.

You can also use negative values to place on the opposite side of the screen

Additionally some css-like evaluation can be used by passing a string adding or subtracting different types. Either use a more literal "50%" to indicate a 50% percentage, or outright calculate "=50% -30"

More methods coming soon!

```lua
im:line(0,0,200,200) -- line from 0,0 pixels to 200,200 pixels
im:line(0.1,0,0.9,0) -- line from 10% of the image on the X position to 90% of the image
im:line("%10",0,"%90",0) -- same thing, 10% x to 90% x
im:rect("=%50 - 32","=%50 -32",64,64) --- draw 64x64 rectangle perfectly centered
```

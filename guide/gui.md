### gui

_get gui draw target_

```lua
---@type image
gui
```

There are two special image instances that have direct ties to textures, editing of which will show immediately without further commands. `gui` operates on the the front screen and overlaps the 3d scene, [sky](#sky) operates on the flat skybox behind the scene.

`gui` unlike `sky` is assumed to be the default choice when draw methods such as `line` or `fill` are used. As such calling them without an image acts as an alias. For instance `line` is the same as `gui:line`

```lua
gui:fill("F00")
gui:line(0.,1.,1.,0.,"FFF") -- draw  white on gui
line(0.,0.,1.,1.,"000") -- draw black line on gui
clr() --clear gui
```

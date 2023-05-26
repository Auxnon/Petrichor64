## anim

_set animation_

```lua
---@type fun(name:string, items:string[], speed?:integer)
function anim(name,items,speed)
```

Provide with a series of textures to create a sequence that a sprite entity can easily animate with (Could apply to a 3d mesh as well if your UVs match up). The optional speed integer is a simple multipler that currently only supports integers.

To actually use the animation you must use the anim method on an entity `entity:anim("animation")`. This has no correlation on what the entities current texture is. See (entity)[entity] for more details.

You can also set the texture of an entity manually every frame but this is needlessly more complex unless you need more granular control

> _Bonus_: You can set animations including ping-pong with a json file as outputed by an aseprite made animation. A more robust universal config format, as well as more command parameters, is still WIP

```lua
anim("walk",{"walk1","walk2","walk3"})
ent:anim("walk")
```

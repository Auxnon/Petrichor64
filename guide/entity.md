### entity (userdata)

Created with the [make](#make) command an entity represents either a billboarded sprite or mesh with mutable position, rotation, scale, etc.

The following fields directly modify the entity when set:

- `x: number` - X position
- `y: number` - Y position
- `z: number` - Z position
- `rx: number` - X axis rotation
- `ry: number` - Y axis rotation
- `ry: number` - Z axis rotation
- `scale: number` - Scale with 1.0 being the default
- `vx: number` - No direct correlation, a convenience field
- `vy: number` - No direct correlation, a convenience field
- `vz: number` - No direct correlation, a convenience field
- `flipped: boolean` - Simple value to flip a sprite or mesh on it's X axis, temporary solution will be deprecated in the future
- `offset: number[3]` - A positional offset to ease the usage of models with awkward origins to their use case
- `tex: string` - A means to set the texture used directly without concern of whether it's interpreted as a model or not
- `asset: string` - The same logic used on entity creation, can be interpreted as a model, and failing that will fallback to using as a billboarded plane with the named texture if found
- `id: integer` - read only. This is provided by the renderer and can always be used by the [kill](#kill) command to remove the rendered counterpart even if the lua version is lost

**Methods**

- `anim(animation: string, force?:boolean)` - set an animation created from config or via the [anim](#anim) command
- `kill` - direct [kill](#kill) usage, destroys object from the renderer

```lua
ent=make()
ent.offset={0,0,2} -- will always appear 2 units higher on the z axis then normal
ent.x=ent.x+10 -- increment x position by 10
ent.rz=tau/4 -- rotate on the z axis 45 degrees
ent.scale = 10 -- 10 times larger
ent:anim("walk") -- set walk animation, does not reset interval if already set
ent:anim("die",true) -- force animation to start at 0, necessary for animations set to once
ent.asset="cube" -- change to a cube model if available
ent.tex="grass" -- set the texture to grass if available, but will not alter a model set prior
ent:kill() -- 💀
```

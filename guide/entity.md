## entity (userdata)

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
- `hit_shape: integer` - collision shape override for [hit](#hit): `0` = auto (cylinder for a billboard sprite, a box baked from the model's bounds otherwise), `1` = box, `2` = cylinder
- `hit_size: number[3]` - collision half-extents override, local space: `{x, y, z}` for a box, or `{radius, radius, half_height}` for a cylinder (only the first and third entries are read). `{0, 0, 0}` (the default) means "use the computed default" instead
- `hit_offset: number[3]` - collision center offset override, local space, on top of `hit_size`
- `tint: number[4]` - vertex-colour tint, rgba 0..1, multiplied into the entity's shaded output. `{1,1,1,1}` (the default) is a no-op. Plain numbers only — not the hex-string/0..255-table forms `fog`/`lum`'s colours accept

**Methods**

- `anim(animation: string, force?:boolean, speed?:number)` - set an animation created from config or via the [anim](#anim) command. On an entity whose model is a skinned glTF mesh, a name matching one of the model's clips plays that clip instead (looping; `speed` scales it, `""` returns to the rest pose). See **Skeletons** below
- `kill` - direct [kill](#kill) usage, destroys object from the renderer

**Skeletons**

An entity made from a glTF model that has a skin (an armature) is skinned on the
GPU and can be posed from script. Its clips are the glTF animations, by name
(lowercased). The pose is a function of the clip, how long it has played, and
any bone overrides, so `ent:bone` reads exactly what is drawn.

- `bones: string[]` - read only. The skeleton's joint names, parents first
  (empty if the model has no skeleton)
- `ent:bone(name: string): {x, y, z, rx, ry, rz}|nil` - the bone's world
  position (tile units, like `ent.x`) and its current override rotation; `nil`
  if there's no such bone
- `ent:bone(name, rot: number[3], pos?: number[3])` - override a bone in its own
  local space, on top of whatever clip is playing: `rot` = `{rx, ry, rz}` radians
  multiplied into the animated rotation, `pos` added to its translation. Returns
  the same table as the getter
- `ent:bone(name, false)` - clear that bone's override

Vertex data is used as authored, so export skinned models **Z-up** (Blender:
untick "+Y Up"). Up to 64 joints per skin. A skin without inverse bind
matrices gets them from its rest pose. Builds with no GPU (headless, terminal)
still load skeletons, so bone queries work there too.

```lua
guy = make("player", 0, 0, 0)
guy:anim("walk")                     -- clip from player.glb
guy:bone("head", {0, 0, 0.5})        -- look left while walking
local hand = guy:bone("hand.r")
sword.x = hand.x
sword.y = hand.y
sword.z = hand.z
guy:bone("head", false)              -- let the clip drive it again
```

**Entity-owned tile grids**

Any entity can own its own tile grid — an independent set of tiles that moves
and rotates with the entity instead of being locked to the static world grid.
Riding one is a script decision (read a [hit.grid](#hitgrid) result yourself,
`group()` if you want it to carry a passenger) — nothing here auto-attaches
anything. These mirror the world's own `tile`/`dtile`/`istile`/`gtile`/`ftile`
natives exactly, just scoped to `self` (a grid-less entity's query methods
return `nil`, not an error):

- `ent:tile(asset: string, x, y, z: integer, rot?: integer)` - set (or, with an
  empty `asset`, clear) one tile in this entity's own grid, in the entity's
  local tile-index space
- `ent:dtile(x?, y?, z?: integer)` - drop one local chunk (32×32×32 tiles), or
  with no args, clear the whole grid
- `ent:istile(x, y, z: integer): boolean|nil` - is there a tile at this local
  index (`nil` if this entity has no grid)
- `ent:gtile(x, y, z: integer): string|nil` - the tile's asset name (`""` if
  empty, `nil` if this entity has no grid)
- `ent:ftile(target: string, x, y, z, dx, dy, dz: integer): number[3]|table` -
  step from `(x,y,z)` by `(dx,dy,dz)` up to 100 times, return the first local
  index matching `target` (or, with `target = ""`, the first empty cell); `{}`
  if nothing is found

A grid's tiles stay a fixed 16-unit cube, same as the world grid — they don't
scale with `size`/`scale`/`offset`. See [hit.grid](#hitgrid) for how other
entities' colliders test against a grid, and `src/tile_grid.rs` for the
implementation notes.

```lua
platform = make('cube', 0, 0, 0, 1)
platform:tile('cube', 0, 0, 0, 0)
platform:tile('cube', 1, 0, 0, 0)
platform.rz = tau / 8 -- the whole grid tilts with the entity

for _, h in ipairs(hit.grid(player.id)) do
  if h.owner == platform.id then
    player.x = player.x + h.normal[1] * h.depth
  end
end
```

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

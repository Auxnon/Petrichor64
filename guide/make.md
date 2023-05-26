## make

_create entity_

```lua
---@type fun(asset?:string, x?:number, y?:number, z?:number, scale?:number)
function make(asset,x,y,z,scale)
```

Create an entity instance with optional asset at position. Asset will attempt to locate a model or mesh named as such, and failing this will fallback to locating a texture with the same name instead and applying as a billboarded sprite. This is similar logic to [tile](#tile) except tile will default to a textured cube instead. Asset or texture can always be modified after the fact with the `asset` or `tex` fields. Refer to [entity](#entity) for methods.

```lua
sprite=make("example",10,0,0) -- sprite with example asset at X 10
```

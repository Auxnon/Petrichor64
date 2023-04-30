### tile

_set tile_

```lua
tile(asset_name,x,y,z,rotation)
```

Set a cube or defined model in the world at a position x,y,z. The provided asset string will first check for a model with the matching name that was either preloaded or defined by the the script before this method was called. If this it will then check for a texture that was preloaded or defined by script. If it only finds a texture it will create a cube textured on each side with the image.

Regardless of the asset chosen it will place it within the world with 0 rotation applied. If you desire a different orientation it's limited to facing 1 of 6 directions by providing a 4th integer to the command

0. default
1. 90 degrees on z axis
2. 180 degrees on z axis
3. 270 degrees on z axis
4. TODO
5. TODO

```lua
tile("example",1,2,3,2) --- place example asset at position 1,2,3 and rotated 180 degrees
```

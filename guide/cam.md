### cam

_set camera_

```lua
--- x,y,z or azimuth altitude
---@type fun({pos?:number[], rot?:number[]})
function cam(params)
```

Move the camera position within the world by passing a table with an index named `pos` containing 3 numbers representing position. The vector is numeric not named, i.e. `{1,2,3}` is at x 1, y 2, z 3. To rotate the camera pass a table with an index named `rot`. The rotation system is work in progress and assumes an up vector of Z+ for now. An azimuth of 0 is X+, Tau/4 (Pi/2) is Y+. Rotate the camera on the azimuth on the z axis for the first number (rot[1]) and look up or down an altitude angle for the second number (rot[2])

```lua
cam {pos={0,0,10}} -- move to Z 10
cam({rot={pi/2}}) -- look down positive Y
cam {rot={0,-tau/8}} -- look down by 45 degrees
cam {pos={-10,0,0},rot={0,tau/4}} -- move to -10 X and look straight up
```

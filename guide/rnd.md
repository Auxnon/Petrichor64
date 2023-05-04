### rnd

_random floating point value_

```lua
---@type fun(a?:number, b?:number):number
function rnd(a, b) end
```

Returns a random number either between a provided range betwen parameters A and B, a single parameter A from 0. - A, or with no parameters returns 0. - 1.

```lua
rnd(-5,5) -- -5.0 to 5.0
rnd(100) -- 0.0 to 100.0
rnd() -- 0.0 through 1.0
```

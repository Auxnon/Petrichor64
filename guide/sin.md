### sin

_sine value_

```lua
---@type fun(n:number):number
function sin(n)
```

Mathematical sine function. Uses standard rust library not lua's math library. See [cos](#cos) for cosine.

```lua
assert(sin(pi) == 0)
assert(abs(sin(tau))> 1e-16) -- very small margin of error
assert( sin(tau/2) - cos(pi/4)==0 ) --tau is pi*2
```

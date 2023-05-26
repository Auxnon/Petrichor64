## cos

_cosine value_

```lua
---@type fun(n:number):number
function cos(n)
```

Mathematical cosine function. Uses standard rust library not lua's math library. See [sin](sin) for sine.

```lua
assert(cos(pi) == -1)
assert(cos(tau) == 1)
assert( abs(1/sqrt(2) - cos(pi/4)) < 0.000000025 )
```

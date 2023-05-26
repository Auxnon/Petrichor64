## irnd

_random integer_

```lua
---@type fun(a?:integer, b?:integer):integer
function irnd(a, b) end
```

Identical to [rnd](#rnd) except always returns an integer.

```lua
rnd(-5,5) -- -5 to 5
rnd(1,20) -- D20
rnd(100) -- 0 to 100
rnd() -- -9,223,372,036,854,775,807 to 9,223,372,036,854,775,807
```

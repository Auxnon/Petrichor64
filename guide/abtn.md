### abtn

_check analog button_

```lua
--- @type fun(button:string): number
function abtn(button)
```

Similar to [btn](#btn) instead of returning a simple boolean we can get the analog value between -1 and 1. Usually for joysticks this could theoretically support any controller with this level of sensitivty. A joystick will return a negative value when pulled left and if pulled towards you (down), and positive when pulled right or if pushed away (upward).

```lua
-- rough example of adding a controller deadzone
local lstick=abtn("lstick")
if lstick>0.5 or lstick<-0.5 then
    x_movement=x_movement+lstick*speed
end
```

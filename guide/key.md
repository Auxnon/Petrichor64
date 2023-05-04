### key

_check keyboard key_

```lua
---@type fun(name:string, volatile?:boolean):boolean
function key(name)
```

Passing a case insenstive string representing a single keyboard character will return whether it's currently held down or not. Including an optional boolean of true will return a whether the key is pressed during a single game tick, then that key must be released before it can be detected again. This is useful when you're concerned with a single tap without adding checks on if a key was pressed in a previous tick.

If a key can be typed to represent itself as a symbol this is how the key is checked, such as ";" for the semicolon key. left and right duplicate keys are available as, for instance, "lctrl" and "rctrl" for left and right control.

```lua
if key("w") then -- valid as long as key is pressed down
    forward()
end

if key("space",true) then  -- works once then must release key to detect again
    jump()
end
```

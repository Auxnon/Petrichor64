### cout

_character output_

```lua
---@param ... string|number|integer|boolean|image|entity|connection|nil
function cout(...)
```

Similar to the standard print method, this will output text to the in-engine console when you press backtick/tilde. For compatability reasons, lua's default print method is overriden to call this instead.

```lua
cout "console message!"
cout("list things: ",variable,entity,image)
```

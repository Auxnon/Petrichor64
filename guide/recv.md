## host.recv

_the next string from the host app_

```lua
---@type fun():string|nil
function host.recv()
```

The oldest message the host app sent that the cart hasn't read yet, or `nil`
when there are none (see [host](#host)). It never waits, so call it in a loop
each frame until it gives `nil`. Messages sent before the cart loaded are kept
for it.

```lua
local msg = host and host.recv()
while msg do
  say(msg)
  msg = host.recv()
end
```

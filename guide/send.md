## host.send

_send a string to the host app_

```lua
---@type fun(msg:string)
function host.send(msg)
```

Hand a string to the app running this cart (see [host](#host)). It arrives on
the host's side in the order sent. If the host never reads them, only the
newest 1024 are kept.

```lua
if host then host.send('{"clicked":"' .. name .. '"}') end
```

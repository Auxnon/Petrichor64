## connection (userdata)

A object representing an established, failed, or closed connection, made by [conn](#conn).

- `socket:send(data:string|table)` - send data. A string goes as-is; a table is
  encoded as JSON first (a table whose keys are exactly `1..n` becomes an array,
  anything else an object)
- `socket:recv(json?:boolean):string|table|nil` - next received message, `nil` if
  nothing arrived. Pass `true` to decode it from JSON into a table (JSON `null`
  becomes `nil`); a message that isn't valid JSON still comes back as a string
- `socket:test():string|nil` - check status of connection. Will return nil while
  connecting or open, an error message for failures, otherwise 'safe close' when
  closed on purpose or by the other side
- `socket:kill()` - close the connection

```lua
socket:send({event="hello", at={1,2,3}}) -- {"event":"hello","at":[1,2,3]}
msg=socket:recv(true)
if msg then cout(msg.event) end
err=socket:test()
if err then cout("lost connection: "..err) end
```

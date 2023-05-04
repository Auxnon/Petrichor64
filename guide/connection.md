### connection (userdata)

**Work In Progress**

A object representing an established, failed, or closed web socket connection.

- `socket:send(data:string)` - send data
- `socket:recv():string|nil` - recieve data, nil if nothing arrived
- `socket:test():string|nil` - check status of connection. Will return nil if active, an error message for failures, otherwise 'safe close' when closed on purpose or otherwise
- `socket:kill()` - close the connection

```
message=socket:recv()
```

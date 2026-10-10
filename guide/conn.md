## conn

_set connection_

```lua
---@type fun(address:string, udp?:boolean, server?:boolean):connection
function conn(address, udp, server)
```

Opens a [connection](#connection). The address decides the transport:

- **WebSocket** — `ws://host:port/path`, `wss://host/path`, or a path starting
  with `/`. A leading-`/` path is resolved against the page's own origin, so it
  only works on the web build (where it picks `wss:` on an https page). Needs the
  `ws` feature, which the web build always has; native builds opt in with
  `--features ws`. This is the transport to use for talking to an ordinary web
  server, and the only one available in a browser. `udp` and `server` are ignored.
- **TCP** — anything else, e.g. `192.168.1.2:3000`. Raw TCP framed with
  [MessagePack](https://msgpack.org/), native only, behind the `online_capable`
  feature. Set `server` to true to host instead: the easiest server is another
  instance of Petrichor64 (or its headless build) running with `server` set.

Messages sent before a WebSocket finishes opening are queued, so it's fine to
`send` a handshake straight after `conn`. A build with no networking compiled in
returns `nil`.

See [connection](#connection)

```lua
socket=conn("ws://localhost:4000/socket/websocket") -- websocket
socket=conn("/socket/websocket") -- websocket on the page's own host (web only)
socket=conn("192.168.1.2:3000") -- raw tcp
```

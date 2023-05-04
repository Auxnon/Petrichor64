### conn

_set connection_

```lua
---@type fun(address:string, udp?:boolean, server?:boolean)
function conn(address, udp, server)
```

Creates a web socket to specified website or IP address. Currently uses the [MessagePack](https://msgpack.org/) protocol. Easiest way to establish a server is have a seperate instance of Petrichor64 running with the server boolean set to true. Port forwarding may be necessary depending on how users are intended to connect. The client versions of an app or game will then need to know the ip address or website it's hosted on.

You can also use this to pull data from a website directly in theory. Currently this is limited to MessagePack but other payloads like JSON will be supported in the future.

You may want to use the headless version of Petrichor64 if available, this runs without graphical interface from command line.

See [connection]()

```lua
socket=conn("192.168.1.2:3000")

```

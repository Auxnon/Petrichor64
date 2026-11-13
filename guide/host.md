## host

_talk to the app running this cart_

```lua
---@class host
---@field send fun(msg:string)
---@field recv fun():string|nil
host = {}
```

When Petrichor64 runs inside another app (a chat client showing a cart, a
launcher, a web page), that app is the **host**, and `host` is a channel of
strings between it and the cart: [host.send](#hostsend) to tell it something,
[host.recv](#hostrecv) to hear what it said. What the strings mean is up to the
two of you; JSON is a fine choice.

Messages from the host wait in a queue until the cart reads them, including
ones sent before the cart finished loading. Each direction holds at most 1024
messages: past that the oldest is dropped, so read them every frame.

`host` is `nil` when the host doesn't allow it (and in an editor overlay), so
check before using it. Under plain `chor` it exists, but nobody is listening.

```lua
function loop()
  if host then
    local msg = host.recv()
    while msg do
      cout("host says " .. msg)
      msg = host.recv()
    end
  end
end
```

### What a host can turn off

A host can run a cart it doesn't trust with less reach. Whatever it turns off is
simply not there (`nil`), rather than failing when called:

- `conn`, with no network;
- `io` (and `glb`), with no files; with files kept to one folder, `io` paths are
  relative to that folder and can't leave it;
- `io.copy` and ctrl+V pasting, with no clipboard;
- the console, its commands and the editor overlays;
- `sub` and `over`, and `quit(1)` loading another cart, with no other apps
  (`init()`, the cart reloading itself, stays);
- sound: the sound commands are still there but make no sound, and `mic` and
  `midi` do nothing;
- `host` itself.

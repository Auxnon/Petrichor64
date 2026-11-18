## hook

_keyed events to and from the host app_

```lua
---@type fun(key:string, value:string)
function hook(key, value) end -- yours: the engine calls it

---@type fun(key:string, value?:string)
function host.hook(key, value)
```

Besides plain strings ([host.send](#hostsend) and [host.recv](#hostrecv)), the
host channel carries **hook events**: a key that names what happened and a
string value. They suit an app that drives a cart from its own controls, such
as a search field outside the game telling the cart what to show.

**In:** define a global `hook(key, value)` and the engine calls it once for
each event the host sent, in the order sent, before that frame's `loop()`.
There's no need to poll. Both arguments are strings (an event sent with no
value has `""`). A cart with no `hook` simply doesn't hear them. Events sent
before the cart finished loading wait for it, and come after `main()`.

**Out:** `host.hook(key, value)` sends an event to the host, which gets it as
the pair `(key, value)`. `value` defaults to `""`.

A key is at most 256 bytes and a value at most 64 KiB: a longer one from the
host is dropped, and `host.hook` raises an error. As with messages, each
direction holds at most 1024 events and drops the oldest past that.

Hooks go with `host`: when the host doesn't allow the host channel, `host` is
`nil` and `hook` is never called.

```lua
function hook(key, value)
  if key == 'search' then
    S.query = value
  elseif key == 'open' then
    S.open = true
  end
end

function loop()
  if S.picked and host then
    host.hook('picked', S.picked)
    S.picked = nil
  end
end
```

From the host's side: `Instance::hook(key, value)` and `Instance::hooks()` in
Rust; on the web `el.hook(key, value)` on a `<petrichor-64>` element and a
`hook` event whose `detail` is `{ key, value }`.

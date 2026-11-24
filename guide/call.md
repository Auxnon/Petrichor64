## call

_the host app calls your globals by name, and you call it back_

```lua
---@type fun(name:string, ...:string|number|boolean|nil)
function host.call(name, ...)
```

The engine already calls a cart's `main()`, `loop()` and `draw()`. The app
hosting the cart can call **any other global function** the same way, by name,
with arguments. Nothing has to be registered: define the function and it can
be called. This suits an app that drives a cart from its own controls, such as
a search field outside the game telling the cart what to show.

**In:** the host names a global and passes arguments; the engine calls it once
per call, in the order made, before that frame's `loop()`. There's no need to
poll. Arguments are strings, numbers, booleans or `nil`. A cart with no global
of that name simply doesn't hear the call, and nothing is returned to the host
(answer with `host.call`). Calls made before the cart finished loading wait
for it, and come after `main()`.

**Out:** `host.call(name, ...)` asks the host to do the same on its side: it
gets the name and the arguments, and decides what they mean.

Tables don't cross; send JSON in a string for those. A name is at most 256
bytes, a call carries at most 16 arguments and 64 KiB of string between them:
a bigger one from the host is dropped, and `host.call` raises an error. As
with messages, each direction holds at most 1024 calls and drops the oldest
past that.

Calls go with `host`: when the host doesn't allow the host channel, `host` is
`nil` and the host's calls never arrive.

```lua
function search(query, limit)
  S.query = query
  S.limit = limit
end

function open_book()
  S.open = true
end

function loop()
  if S.picked and host then
    host.call('picked', S.picked, S.count)
    S.picked = nil
  end
end
```

From the host's side: `Instance::call(name, args)` and `Instance::calls()` in
Rust (`inst.call("search", ["oak".into(), 3.into()])`); on the web
`el.call("search", "oak", 3)` on a `<petrichor-64>` element and a `call` event
whose `detail` is `{ name, args }`.

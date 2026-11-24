## app.swap

_save a script and reload only what changed_

```lua
---@type fun(path:string, contents:string):{kind:string, msg:string, names:string[]}|nil
function app.swap(path, contents)
```

Writes `contents` to `path` in the app being edited, like
[app.write](#write), then makes the running app pick the change up while
disturbing as little as it can. It answers with what it did:

| `kind` | |
|---|---|
| `"fn"` | Only function bodies changed. Those (`names`) were replaced in place: the app's globals, tables, entities and camera are untouched, `main()` does not run again, and the new bodies run from the next call. |
| `"root"` | Something outside a function changed, so the whole app reloaded, as [app.reload](#reload) does. `msg` says why. |
| `"same"` | Nothing that runs changed (comments, spacing), or the file isn't a script: written, nothing reloaded. |
| `"err"` | The script doesn't compile (`msg` has the line and message). It was still written; the app keeps running what it had. |

A replaced function keeps what it had captured, file-level `local`s included,
and a change inside a nested function reaches the closures already handed out:
each runs the new body on its next call with its own captured values. Adding
lines, comments or spacing around functions is `"same"`.

What reloads the whole app: a change to the file's top-level code (a constant,
a table, a function added, removed or renamed), a changed `local function`,
or a changed function whose global now holds something else.

Only exists inside an overlay.

```lua
local r = app.swap("scripts/main.lua", text)
if r.kind == "fn" then
	say("swapped " .. r.names[1])
end
```

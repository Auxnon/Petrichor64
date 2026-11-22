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
| `"fn"` | The edit stayed inside top-level functions. Only those (`names`) were defined again: the app's globals, tables, entities and camera are untouched, `main()` does not run again, and the new bodies run from the next call. |
| `"root"` | Something outside a function changed, so the whole app reloaded, as [app.reload](#reload) does. `msg` says why. |
| `"same"` | Nothing that runs changed (comments, spacing), or the file isn't a script: written, nothing reloaded. |
| `"err"` | The script doesn't compile (`msg` has the line and message). It was still written; the app keeps running what it had. |

What makes an edit reload the whole app: a change to top-level code, a
function removed or renamed, the same function defined twice, or a changed
function that uses a file-level `local` (defined again on its own it would no
longer share it). A change inside a nested function redefines the top-level
function around it; closures the old one already handed out keep their old
code.

Only exists inside an overlay.

```lua
local r = app.swap("scripts/main.lua", text)
if r.kind == "fn" then
	say("swapped " .. r.names[1])
end
```

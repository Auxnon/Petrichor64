## app.lint

_what's wrong with some source, without running it_

```lua
---@type fun(source:string):table[]
function app.lint(source)
```

Compiles the source without running it and returns every problem found, each
`{line, col, line2, col2, error, msg}`: where it starts and ends (from 1),
whether it's an error rather than a warning, and the message. An empty table
means it compiles.

Only exists inside an overlay.

```lua
local problems = app.lint(text)
if #problems > 0 then
	local p = problems[1]
	status = "line " .. p.line .. ": " .. p.msg
end
```

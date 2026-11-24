## app.lex

_the tokens of some source, for highlighting_

```lua
---@type fun(source:string):integer[]
function app.lex(source)
```

Reads Lua/silt source the way the language server does and returns its
semantic tokens as one flat array, four numbers per token, in source order:
`line, column, length, kind` (lines and columns from 1). Nothing is run.

| kind | | kind | |
|---|---|---|---|
| 1 | keyword | 7 | method |
| 2 | string | 8 | parameter |
| 3 | number | 9 | variable |
| 4 | comment | 10 | property |
| 5 | operator | 11 | type |
| 6 | function | | |

A source that doesn't compile is still tokenized as far as it can be; ask
[app.lint](#lint) what's wrong with it. Only exists inside an overlay.

```lua
local t = app.lex(text)
for i = 1, #t, 4 do
	paint(t[i], t[i + 1], t[i + 2], KIND_COLOR[t[i + 3]])
end
```

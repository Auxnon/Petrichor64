## app.names

_what the app has to offer by name_

```lua
---@type fun():{tex:string[], mod:string[], snd:string[]}|nil
function app.names()
```

The names the edited app can pass to commands that take an asset: `tex` its
textures (image files and ones made with `tex()`), `mod` its models (files,
`mod()` and the built-in `cube`/`plane`), `snd` the sound files in its
`sounds/` folder. Each list is sorted.

Only exists inside an overlay.

```lua
local n = app.names()
for i = 1, #n.tex do
	print(n.tex[i])
end
```

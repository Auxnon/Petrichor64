## app.list

_every file in the app being edited_

```lua
---@type fun():table
function app.list()
```

An array of every file in the app this overlay was opened to edit, as paths relative
to its folder (`scripts/main.lua`), sorted. Subfolders are included; dotfiles and
symlinks are not.

Only exists inside an overlay. This is how an editor builds its file list without
being told what the app contains:

```lua
for i = 1, #app.list() do cout(app.list()[i]) end
```

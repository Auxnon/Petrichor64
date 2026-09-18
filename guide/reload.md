## app.reload

_reload the app being edited_

```lua
---@type fun():boolean
function app.reload()
```

Restart the app this overlay was opened to edit, so files written with `app.write`
take effect. Returns whether there was an app to reload.

The overlay itself is untouched — it is deliberately not a child of the app, so a
reload can't take the editor down with it and your cursor, buffer, and scroll
position all survive.

Only exists inside an overlay.

```lua
app.write("scripts/main.lua", src)
app.reload()
```

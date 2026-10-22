## app.hide

_hide a tile or entity while it's edited_

```lua
---@type fun(hit:table, on?:boolean):boolean
function app.hide(hit, on)
```

Takes what [app.pick](#pick) returned. An entity stops being drawn and picked
(the app's own code still sees it as it was); a tile is emptied out of the
app's world and put back as it was. `app.hide(hit, false)` shows it again, and
everything an overlay hid comes back when it closes.

The model editor (Ctrl+M) hides the thing it opened, so the copy it edits in
place isn't drawn through by the original. Returns whether it worked.

Only exists inside an overlay.

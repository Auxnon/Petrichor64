## app.close

_dismiss this overlay_

```lua
---@type fun()
function app.close()
```

Closes the overlay that calls it, and resumes the app if no other overlay is
open. Only exists inside an overlay.

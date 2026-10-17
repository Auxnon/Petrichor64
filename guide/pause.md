## app.pause

_freeze the app underneath_

```lua
---@type fun(on?:boolean)
function app.pause(on)
```

Stops calling the app's `loop` (`app.pause(false)` resumes it), so the scene
holds still while an overlay inspects or edits it. Rendering carries on.
Closing the last overlay always resumes the app, so an editor that forgets —
or errors out — can't leave it frozen.

Only exists inside an overlay.

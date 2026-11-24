## io.open

_ask for a file with the system's file dialog_

```lua
---@type fun()
function io.open()
```

Shows the file dialog of whatever the engine is running on: the desktop's own
(on Linux through `zenity` or `kdialog`), or Android's document picker. It
returns at once. If the person chooses a file, its path arrives at the app's
global `drop(path)`, exactly as if the file had been dragged onto the window;
if they cancel, nothing happens.

```lua
function drop(path)
    cout("got " .. path)
end

function loop()
    if key("o", true) then
        io.open()
    end
end
```

On Android the path is a copy of the chosen document in the app's cache.
Nothing happens in the terminal build or the browser, and the function is
`nil` when the app isn't allowed to load other apps (`Policy::load_apps`),
so check for it: `if io and io.open then`.

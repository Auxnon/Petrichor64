## dimg

_open an image dropped on the window_

```lua
---@type fun(path:string, max?:integer):image?
function dimg(path, max)
```

When a file is dropped on the window (from a file manager), the engine calls
the app's global `drop(path)` with its full path, or, while an overlay is
open, the overlay's. `dimg` opens such a file as an [image](#image), shrunk
to fit `max` pixels (default 512, 8 to 2048) on its longer side, to draw on or
hand to [tex](#tex).

Only a path `drop` was given can be opened: `dimg` returns nil for any other
path, and for a file that isn't an image (png, jpg, gif, bmp, webp...). There
is nothing to drop on the web, where it's always nil.

```lua
function drop(path)
	local im = dimg(path, 256)
	if im ~= nil then
		tex("photo", im)
		make("photo", 0, 0, 0)
	else
		cout("not an image: " .. path)
	end
end
```

## touches

_every finger on the screen_

```lua
---@return {id:integer, x:number, y:number, pen:boolean, pressure:number}[]
function touches()
```

Returns a list with one entry per finger on the screen, empty when nothing is
touching (and always empty on a machine with no touchscreen).

- `id` - a number from 1 to 5 that stays the same while the finger is down, so a
  finger can be followed from tick to tick. A lifted finger's id is reused by the
  next one to land.
- `x`, `y` - where it is, 0 to 1 across and down the screen, as in `mus()`
- `pen` - true for a stylus (an active pen on Android, an Apple Pencil on iOS),
  false for a finger, so an app can draw with the pen and navigate with fingers
- `pressure` - 0 to 1. A screen that can't sense it reports 1 (or 0) throughout.

Up to five fingers are reported. The first finger down also drives `mus()` (its
`x`, `y` and `m1`), so a game that only needs one finger can ignore this; a
second finger never moves the mouse.

Two-finger pinch and drag, measured from tick to tick:

```lua
local last
function loop()
	local t = touches()
	if #t == 2 then
		local cx, cy = (t[1].x + t[2].x) / 2, (t[1].y + t[2].y) / 2
		local d = math.sqrt((t[1].x - t[2].x) ^ 2 + (t[1].y - t[2].y) ^ 2)
		if last then
			zoom = zoom * last.d / d   -- spread apart: zoom in
			pan(cx - last.cx, cy - last.cy)
		end
		last = { cx = cx, cy = cy, d = d }
	else
		last = nil
	end
end
```

A finger that's part of a gesture is still the mouse if it landed first, so a
game that reads both should stop treating `m1` as a click once a second finger
lands.

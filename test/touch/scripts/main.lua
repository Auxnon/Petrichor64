-- Pointer + unprojection smoke test.
--
-- On a phone the primary finger drives mus(); on desktop it's the mouse. Same
-- code either way, which is the point of folding touch into the mouse rather
-- than giving it its own API.
--
-- Two things under test:
--  1. `mus().x/.y` — a crosshair follows the pointer (2D, screen space).
--  2. `mus().vx/.vy/.vz` — a cube placed `dist` units out along the ray through the
--     pointer. If unprojection handles this screen's aspect correctly the cube sits
--     *under the crosshair* and stays there while dragging. Drift away from the
--     crosshair is the failure to watch for, and a phone's very tall aspect ratio is
--     exactly where that shows up.
--
-- The ray is normalised and points *away* from the camera, into the scene: at screen
-- centre it reads (0, 1, 0), which is the camera's forward. (The commented-out
-- `dir * -16` in src/ray.rs suggests it once pointed the other way — don't trust it.)
--
-- Android: `just android-apk test/touch && just android-install && just android-log`

dist = 12.0
cube = nil
was_down = false

function main()
	-- Dark sky, so the cube reads against it. Without this the background is the
	-- uninitialised magenta, which is easy to mistake for a render failure.
	sky:fill("113")
	cube = make("cube", 0., dist, 0.)
	-- A bare `make("cube", ...)` is invisible: the mesh needs a texture, and it's
	-- corner-anchored, so without the offset it hangs off the point rather than
	-- centring on it — which reads as "unprojection is wrong" when it isn't.
	cube.tex = "example"
	cube.size = { 1.5, 1.5, 1.5 }
	cube.offset = { -0.5, -0.5, -0.5 }
	cout("unprojection test: the cube should sit under the crosshair")
end

function loop()
	local m = mus()
	-- m1 arrives as a boolean, not the 0/1 the engine stores internally.
	local down = m.m1

	-- Place the cube along the pointer ray.
	cube.x = m.vx * dist
	cube.y = m.vy * dist
	cube.z = m.vz * dist

	cam({ pos = { 0., 0., 0. }, rot = { tau / 4, 0. } })

	-- clr(), not fill(): fill paints the gui layer *opaque*, which hides the 3D
	-- scene behind it — so the cube would never be visible. Clearing each frame is
	-- also what stops the crosshair from smearing into a grid of old positions.
	clr()
	local c = "0F0"
	if down then
		c = "F00"
	end
	-- Full-width/height lines, so the position reads clearly on a screen whose
	-- aspect ratio the app knows nothing about.
	line(m.x, 0., m.x, 1., c)
	line(0., m.y, 1., m.y, c)

	-- Edges only: printing every frame at 60fps floods the log.
	if down ~= was_down then
		was_down = down
		local state = "up"
		if down then
			state = "down"
		end
		cout(
			"touch "
				.. state
				.. " screen="
				.. m.x
				.. ","
				.. m.y
				.. " ray="
				.. m.vx
				.. ","
				.. m.vy
				.. ","
				.. m.vz
		)
	end
end

function draw() end

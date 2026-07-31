-- Overlay demo — proof that a second bundle draws over a running app and owns input.
--
-- Spawn it from the engine console while a game is running:
--
--     overlay apps/overlay-demo
--     overlay off
--
-- It is NOT launchable by the app underneath, and that's deliberate: overlays are the
-- privileged surface (filesystem access, editing another bundle), so only the engine
-- may summon one. See PLAN.md's overlay trust boundary.
--
-- What it demonstrates:
--   * this bundle draws to its own gui layer, composited above the app's
--   * the pixels it leaves clear show the app straight through
--   * it receives input; the app underneath receives none while it's up
--
-- Note the compositing is alpha-*tested*, not blended: a pixel is either this
-- layer's or the app's. So the panel below is drawn solid, and the area around it is
-- left untouched rather than tinted — a dimmed backdrop isn't possible without a
-- `mix()` in gui_fs_main.

presses = 0
last_key = "-"
was_down = false

KEYS = { "a", "b", "c", "d", "e", "f", "g", "h", "i", "j" }

function main()
	cout("overlay demo up — this bundle owns input while it's on screen")
end

function loop()
	local m = mus()
	local i = 0

	-- Count keypresses to show input is arriving here and not at the app.
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			last_key = KEYS[i]
			presses = presses + 1
		end
	end
	if m.m1 and not was_down then
		presses = presses + 1
		last_key = "click"
	end
	was_down = m.m1

	clr()

	-- A panel, deliberately not full-screen: everything outside it stays clear, so
	-- the app underneath is visible and you can see the two layers coexisting.
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:rect(0.06, 0.06, 0.88, 0.012, "5CF")
	gui:text("OVERLAY  bundle 1 / secondary layer", 0.09, 0.11, "DEF")
	gui:text("input owner: this overlay", 0.09, 0.16, "9CE")
	gui:text("presses: " .. tostring(presses) .. "   last: " .. last_key, 0.09, 0.21, "FE9")
	gui:text("pointer: " .. tostring(flr(m.x * 100)) .. "," .. tostring(flr(m.y * 100)), 0.09, 0.26, "9E9")
	gui:text("` for console, then: overlay off", 0.09, 0.31, "888")

	-- A pointer mark, to show the overlay tracks input while the app cannot.
	line(m.x - 0.02, m.y, m.x + 0.02, m.y, "F5C")
	line(m.x, m.y - 0.02, m.x, m.y + 0.02, "F5C")
end

function draw() end

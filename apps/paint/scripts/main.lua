-- FRESCO — paint on a canvas that hangs in 3D space.
--
-- The canvas is not a screen overlay: it's a real object in the world, and the
-- brush position comes from unprojecting the pointer onto its plane. Mouse and
-- finger take exactly the same path, because touch drives mus() too.
--
-- Three ideas worth knowing about before reading on:
--
-- 1. THE CANVAS FITS ANY SCREEN, measured rather than assumed. Nothing in the Lua
--    API reports the window's aspect ratio, which matters here — a phone is far
--    taller than a desktop is wide, and a fixed-size canvas would either overflow
--    or float in the middle of the screen. But the pointer ray *is* a probe: for a
--    perspective camera, vx/vy = (2*m.x - 1) * tan(fovx/2), so a single pointer
--    sample solves for the field of view exactly. `fit_canvas` does that and sizes
--    the canvas to fill 90% of whatever screen it finds itself on.
--
-- 2. DRAWINGS SAVE AS STROKES, NOT PIXELS. `io.set` writes UTF-8 text only, so a
--    bitmap was never an option — which turns out to be the better design anyway:
--    a stroke list is small, replays at any canvas resolution, and can be edited by
--    hand. Records are fixed-width because silt has no string.find/gmatch, so
--    parsing is `string.sub` at known offsets.
--
-- 3. PIXELS VS PERCENT. The engine's `gunit` reads an *integer* as a pixel and a
--    *float* as a fraction of the target. Canvas drawing therefore runs everything
--    through `flr()`, which returns an integer; a stray float would silently become
--    a percentage and land somewhere unrelated.
--
-- Controls: drag on the canvas to paint. The tray along the bottom holds colours,
-- brushes, brush size, and save/load/clear. On a keyboard: 1-8 pick colours,
-- q/w/e/r pick brushes, [ and ] change size, s saves, l loads, c clears.

CANVAS_TEX = "fresco_canvas"
FRAME_TEX = "fresco_frame"
SAVE_FILE = "fresco.txt"

PAPER = "EEE9DD" -- warm off-white; a flat FFF canvas looks like a bug, not paper
DIST = 9.0 -- how far in front of the camera the canvas hangs
FILL = 0.9 -- how much of the screen it should cover once fitted
LONG_EDGE = 224 -- canvas image pixels on its longer axis

-- Palette. Eight is enough to be expressive and few enough to hit with a thumb.
COLORS = { "111", "FFF", "D33", "F83", "FC4", "4B5", "39C", "83D" }
-- Brush kinds, in tray order. The letter is also what the save format stores.
KINDS = { "p", "i", "s", "e" }
KIND_NAME = { p = "pen", i = "ink", s = "spray", e = "erase" }
SIZES = { 1, 3, 7, 14 }

canvas = nil -- the image we paint into
canvas_ent = nil -- the plane it's textured onto
frame_ent = nil
half_w = 1.0 -- canvas half-extents in world units, set by fit_canvas
half_h = 1.0
img_w = LONG_EDGE -- canvas image size, fixed once fitted
img_h = LONG_EDGE
fitted = false

color_i = 1
kind_i = 1
size_i = 2
strokes = {} -- fixed-width records, for saving
buttons = {}

drawing = false -- a stroke is in progress (started on the canvas, not the tray)
was_down = false
last_x = 0 -- previous point of the stroke, in canvas pixels
last_y = 0
dirty = false -- canvas changed this frame and needs re-uploading
status = ""
status_hold = 0

-- ---------------------------------------------------------------------------
-- helpers
-- ---------------------------------------------------------------------------

-- Zero-padded 3-digit integer, for the save format's fixed-width fields.
function pad3(n)
	local v = flr(n)
	if v < 0 then
		v = 0
	end
	if v > 999 then
		v = 999
	end
	if v < 10 then
		return "00" .. tostring(v)
	end
	if v < 100 then
		return "0" .. tostring(v)
	end
	return tostring(v)
end

function pad2(n)
	local v = flr(n)
	if v < 0 then
		v = 0
	end
	if v > 99 then
		v = 99
	end
	if v < 10 then
		return "0" .. tostring(v)
	end
	return tostring(v)
end

function say(msg)
	status = msg
	status_hold = 150 -- frames (~2.5s)
	cout(msg)
end

function cur_color()
	return COLORS[color_i]
end

function cur_kind()
	return KINDS[kind_i]
end

function cur_size()
	return SIZES[size_i]
end

-- ---------------------------------------------------------------------------
-- canvas geometry
-- ---------------------------------------------------------------------------

-- Solve the camera's field of view from one pointer sample, then size the canvas
-- to fill the screen. See note 1 at the top.
--
-- Needs the pointer to be off-centre on the axis being measured (dead centre gives
-- 0/0), which is why each axis is guarded and fitting can happen over several
-- frames or not at all until the pointer moves.
function fit_canvas(m)
	if m.vy < 0.2 then
		return false -- pointer is off to the side; the solve is ill-conditioned
	end
	local sx = 2.0 * m.x - 1.0
	local sy = 1.0 - 2.0 * m.y
	if abs(sx) < 0.08 or abs(sy) < 0.08 then
		return false
	end
	local tan_x = (m.vx / m.vy) / sx
	local tan_z = (m.vz / m.vy) / sy
	if tan_x <= 0.0 or tan_z <= 0.0 then
		return false
	end
	half_w = DIST * tan_x * FILL
	half_h = DIST * tan_z * FILL
	return true
end

-- Canvas image dimensions, once we know its shape: longer edge fixed, shorter one
-- proportional, so pixels stay square whatever the screen.
function pick_image_size()
	if half_w >= half_h then
		img_w = LONG_EDGE
		img_h = flr(LONG_EDGE * (half_h / half_w))
	else
		img_h = LONG_EDGE
		img_w = flr(LONG_EDGE * (half_w / half_h))
	end
	if img_w < 16 then
		img_w = 16
	end
	if img_h < 16 then
		img_h = 16
	end
end

function build_canvas()
	pick_image_size()
	canvas = nimg(img_w, img_h)
	canvas:fill(PAPER)
	tex(CANVAS_TEX, canvas)

	local fr = nimg(8, 8)
	fr:fill("221")
	tex(FRAME_TEX, fr)

	-- A thin box rather than a true plane: the cube mesh always exists, and a
	-- little depth makes it read as a physical canvas instead of a floating decal.
	canvas_ent = make("cube", 0., DIST, 0.)
	canvas_ent.tex = CANVAS_TEX
	canvas_ent.size = { half_w * 2., 0.06, half_h * 2. }
	canvas_ent.offset = { -0.5, -0.5, -0.5 }

	-- Backing board, a touch larger and set behind, to give the canvas an edge.
	frame_ent = make("cube", 0., DIST + 0.05, 0.)
	frame_ent.tex = FRAME_TEX
	frame_ent.size = { half_w * 2. + 0.18, 0.06, half_h * 2. + 0.18 }
	frame_ent.offset = { -0.5, -0.5, -0.5 }
	fitted = true
end

-- Where the pointer ray meets the canvas plane, in canvas pixels. Sets hit_x/hit_y
-- and returns true, or returns false if the ray misses the canvas.
--
-- The camera sits at the origin looking down +y with z up, so the plane is just
-- y = DIST and the intersection is one division — no need for a general
-- ray/plane routine while the canvas faces the viewer.
function canvas_hit(m)
	if m.vy < 0.001 then
		return false
	end
	local t = DIST / m.vy
	local hx = m.vx * t
	local hz = m.vz * t
	local u = (hx + half_w) / (half_w * 2.)
	local v = 1.0 - (hz + half_h) / (half_h * 2.)
	if u < 0.0 or u > 1.0 or v < 0.0 or v > 1.0 then
		return false
	end
	hit_x = u * img_w
	hit_y = v * img_h
	return true
end

-- ---------------------------------------------------------------------------
-- painting
-- ---------------------------------------------------------------------------

-- Every `local` in this file is declared at the top of its function, never inside an
-- `if` or a loop body, and that is deliberate: silt mis-resolves a local declared in
-- a nested block, reading a slot that held unrelated userdata and failing with
-- "Value 'Value: userdata' is not callable" pointing at the variable. Declaring up
-- front and assigning later avoids it entirely.
function stamp(ix, iy, s, col, kind)
	local n = 0
	local a = 0.0
	local r = 0.0
	local h = 0
	if kind == "s" then
		-- Spray: scattered points, so density builds with dwell time the way an
		-- airbrush does. Count scales with size or big brushes look sparse.
		n = 3 + flr(s / 2)
		for i = 1, n do
			a = rnd() * tau
			r = rnd() * s
			canvas:pixel(flr(ix + cos(a) * r), flr(iy + sin(a) * r), col)
		end
		return
	end
	if s <= 1 then
		canvas:pixel(flr(ix), flr(iy), col)
		return
	end
	h = flr(s / 2)
	-- rrect with a full-radius corner gives a round nib, so diagonal strokes don't
	-- come out with square shoulders.
	canvas:rrect(flr(ix - h), flr(iy - h), flr(s), flr(s), h, col)
end

-- Paint a segment by stamping along it. Without this, a fast drag would leave a
-- dotted trail — the pointer only reports once per frame.
--
-- The brush is passed in rather than read from the current selection, so replaying a
-- saved file drives exactly the same code with the brush each record carries.
function paint_seg(x1, y1, x2, y2, col, kind, s)
	local ink = col
	if kind == "e" then
		ink = PAPER
	end
	local dx = x2 - x1
	local dy = y2 - y1
	local steps = flr(abs(dx))
	local sy = flr(abs(dy))
	local t = 0.0
	if sy > steps then
		steps = sy
	end
	-- One stamp per pixel of travel; capped so a huge jump can't stall a frame.
	if kind ~= "s" then
		steps = flr(steps / 2)
	end
	if steps < 1 then
		steps = 1
	end
	if steps > 180 then
		steps = 180
	end
	for i = 0, steps do
		t = i / steps
		stamp(x1 + dx * t, y1 + dy * t, s, ink, kind)
	end
	dirty = true
end

-- Paint with the current selection and record it. Erase strokes are recorded too,
-- so replaying a file reproduces the picture rather than an un-erased version of it.
function paint_and_record(x1, y1, x2, y2)
	local col = cur_color()
	local kind = cur_kind()
	local s = cur_size()
	paint_seg(x1, y1, x2, y2, col, kind, s)
	local rec = kind .. col .. pad2(s) .. pad3(x1) .. pad3(y1)
	rec = rec .. pad3(x2) .. pad3(y2)
	strokes[#strokes + 1] = rec
end

function clear_canvas()
	canvas:fill(PAPER)
	strokes = {}
	dirty = true
end

-- ---------------------------------------------------------------------------
-- save / load  (see note 2 at the top)
-- ---------------------------------------------------------------------------

function save_drawing()
	local out = ""
	for i = 1, #strokes do
		out = out .. strokes[i] .. "\n"
	end
	if io.set(SAVE_FILE, out) then
		say("saved " .. tostring(#strokes) .. " strokes")
	else
		-- Writing needs a game *directory*; a packed .game.png (or the copy baked
		-- into an APK) has nowhere to put it. Worth saying plainly rather than
		-- failing quietly.
		say("save failed - run from a game folder")
	end
end

function load_drawing()
	local body = io.get(SAVE_FILE)
	if body == nil then
		say("no " .. SAVE_FILE .. " to load")
		return
	end
	canvas:fill(PAPER)
	strokes = {}
	local n = flr(string.len(body) / 19) -- 18 chars + newline
	local o = 1
	local kind = ""
	local col = ""
	local s = 0
	local x1 = 0
	local y1 = 0
	local x2 = 0
	local y2 = 0
	for i = 0, n - 1 do
		o = i * 19 + 1
		kind = string.sub(body, o, o)
		col = string.sub(body, o + 1, o + 3)
		s = tonumber(string.sub(body, o + 4, o + 5))
		x1 = tonumber(string.sub(body, o + 6, o + 8))
		y1 = tonumber(string.sub(body, o + 9, o + 11))
		x2 = tonumber(string.sub(body, o + 12, o + 14))
		y2 = tonumber(string.sub(body, o + 15, o + 17))
		if s ~= nil and x1 ~= nil and y1 ~= nil and x2 ~= nil and y2 ~= nil then
			-- Replayed through the same code the live brush uses, so a loaded
			-- drawing and a drawn one can't diverge.
			paint_seg(x1, y1, x2, y2, col, kind, s)
			strokes[#strokes + 1] = string.sub(body, o, o + 17)
		end
	end
	say("loaded " .. tostring(n) .. " strokes")
	dirty = true
end

-- ---------------------------------------------------------------------------
-- tray  (2D, on the gui layer — screen space, so no unprojection needed)
-- ---------------------------------------------------------------------------

TRAY_TOP = 0.80

function add_button(x, y, w, h, act, val, label, col)
	-- Fields assigned one at a time rather than written as a `{ x = x, ... }`
	-- literal. Single-line constructors are fine (`cam{pos=...}` works), but silt
	-- chokes on one spanning several lines — first complaining about the trailing
	-- comma, then about a key/value pair mid-table. An empty table plus assignments
	-- uses only constructs it definitely handles, and reads no worse.
	local b = {}
	b.x = x
	b.y = y
	b.w = w
	b.h = h
	b.act = act
	b.val = val
	b.label = label
	b.col = col
	buttons[#buttons + 1] = b
end

function build_tray()
	buttons = {}
	local n = #COLORS
	local cw = 1.0 / n
	local cells = 8
	local bw = 0.0
	for i = 1, n do
		add_button((i - 1) * cw, 0.82, cw, 0.08, "color", i, "", COLORS[i])
	end
	-- Second row: four brushes, then size, then the three actions.
	bw = 1.0 / cells
	for i = 1, #KINDS do
		add_button((i - 1) * bw, 0.90, bw, 0.09, "kind", i, KIND_NAME[KINDS[i]], "333")
	end
	add_button(4 * bw, 0.90, bw, 0.09, "size", 0, "size", "333")
	add_button(5 * bw, 0.90, bw, 0.09, "save", 0, "save", "252")
	add_button(6 * bw, 0.90, bw, 0.09, "load", 0, "load", "225")
	add_button(7 * bw, 0.90, bw, 0.09, "clear", 0, "clr", "522")
end

function button_at(mx, my)
	local b = nil
	for i = 1, #buttons do
		b = buttons[i]
		if mx >= b.x and mx <= b.x + b.w and my >= b.y and my <= b.y + b.h then
			return b
		end
	end
	return nil
end

function press_button(b)
	if b.act == "color" then
		color_i = b.val
	elseif b.act == "kind" then
		kind_i = b.val
	elseif b.act == "size" then
		size_i = size_i + 1
		if size_i > #SIZES then
			size_i = 1
		end
	elseif b.act == "save" then
		save_drawing()
	elseif b.act == "load" then
		load_drawing()
	elseif b.act == "clear" then
		clear_canvas()
		say("cleared")
	end
end

function draw_tray(m)
	local b = nil
	local cw = 0.0
	local bw = 0.0
	gui:rect(0., TRAY_TOP, 1., 1. - TRAY_TOP, "0A1")
	for i = 1, #buttons do
		b = buttons[i]
		gui:rect(b.x + 0.004, b.y + 0.004, b.w - 0.008, b.h - 0.008, b.col)
		if b.label ~= "" then
			gui:text(b.label, b.x + 0.012, b.y + 0.028, "DDD")
		end
	end
	-- Selection marks: a bar under the active colour and brush, and the size
	-- printed on its own button, so the current tool is readable at a glance.
	cw = 1.0 / #COLORS
	gui:rect((color_i - 1) * cw + 0.004, 0.902, cw - 0.008, 0.006, "FFF")
	bw = 1.0 / 8
	gui:rect((kind_i - 1) * bw + 0.004, 0.986, bw - 0.008, 0.006, "FFF")
	gui:text(tostring(cur_size()), 4 * bw + 0.012, 0.955, "FF0")

	if status_hold > 0 then
		status_hold = status_hold - 1
		gui:text(status, 0.02, 0.03, "FFE")
	end
end

-- A ring at the pointer showing the brush footprint, so you know how big a mark
-- you're about to make before making it.
function draw_cursor(m)
	if m.y > TRAY_TOP then
		return
	end
	local col = cur_color()
	local px = 0.0
	if cur_kind() == "e" then
		col = "FFF"
	end
	-- Brush size is in canvas pixels; convert to a fraction of the screen so the
	-- ring matches the mark's real size.
	px = (cur_size() / img_w) * 0.5
	gui:line(m.x - px, m.y, m.x + px, m.y, col)
	gui:line(m.x, m.y - px, m.x, m.y + px, col)
	gui:line(m.x - 0.006, m.y, m.x - 0.002, m.y, col)
	gui:line(m.x + 0.002, m.y, m.x + 0.006, m.y, col)
end

-- ---------------------------------------------------------------------------
-- keyboard (desktop convenience; the tray is the only input a phone needs)
-- ---------------------------------------------------------------------------

KEYNUM = { "1", "2", "3", "4", "5", "6", "7", "8" }
KEYKIND = { "q", "w", "e", "r" }

function read_keys()
	for i = 1, #KEYNUM do
		if key(KEYNUM[i], true) then
			color_i = i
		end
	end
	for i = 1, #KEYKIND do
		if key(KEYKIND[i], true) then
			kind_i = i
		end
	end
	if key("]", true) then
		size_i = size_i + 1
		if size_i > #SIZES then
			size_i = #SIZES
		end
	end
	if key("[", true) then
		size_i = size_i - 1
		if size_i < 1 then
			size_i = 1
		end
	end
	if key("s", true) then
		save_drawing()
	end
	if key("l", true) then
		load_drawing()
	end
	if key("c", true) then
		clear_canvas()
		say("cleared")
	end
end

-- ---------------------------------------------------------------------------

function main()
	sky:fill("123")
	build_tray()
	cout("FRESCO - drag to paint. tray at the bottom; keys 1-8 / q w e r / [ ] / s l c")
end

function loop()
	local m = mus()
	cam({ pos = { 0., 0., 0. }, rot = { tau / 4, 0. } })

	-- Fit on the first frame the pointer geometry allows, then build the canvas.
	if not fitted then
		if fit_canvas(m) then
			build_canvas()
			say("canvas " .. tostring(img_w) .. "x" .. tostring(img_h))
		end
		clr()
		gui:text("move the pointer to place the canvas", 0.06, 0.5, "CCC")
		return
	end

	read_keys()

	local down = m.m1
	local b = nil
	if down and not was_down then
		-- Decide once, on the press, whether this is a tray tap or a stroke —
		-- checking every frame would let a stroke that wanders over the tray start
		-- pressing buttons mid-drag.
		b = button_at(m.x, m.y)
		if b ~= nil then
			press_button(b)
			drawing = false
		elseif canvas_hit(m) then
			drawing = true
			last_x = hit_x
			last_y = hit_y
			paint_and_record(hit_x, hit_y, hit_x, hit_y) -- a tap leaves a dot
		end
	elseif down and drawing then
		if canvas_hit(m) then
			paint_and_record(last_x, last_y, hit_x, hit_y)
			last_x = hit_x
			last_y = hit_y
		end
	elseif not down then
		drawing = false
	end
	was_down = down

	-- Re-upload only when the canvas actually changed: tex() pushes the whole
	-- texture atlas to the GPU, which is not something to do on idle frames.
	if dirty then
		tex(CANVAS_TEX, canvas)
		dirty = false
	end

	clr()
	draw_cursor(m)
	draw_tray(m)
end

function draw() end

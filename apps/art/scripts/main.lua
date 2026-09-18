-- ART — a 4-colour canvas with an interlacing dither brush.
--
-- Everything lives in 3D space, not on the 2D gui overlay: the canvas is a
-- plane pushed back into the background, and the tool buttons are small
-- floating cubes hanging closer to the camera, in front of it. A pointer ray
-- is cast from the camera and tested against each plane in turn, nearest
-- first, exactly like FRESCO's canvas_hit but generalised to any depth.
--
-- The palette is fixed at four colours: black, cyan, magenta, yellow — the
-- print primaries, chosen because they interlace into secondary colours
-- (cyan + yellow dithered together reads as green at a glance, etc).
--
-- Two brush kinds:
--   full   — every pixel in the brush footprint is overwritten.
--   dither — only every-other pixel is, in a checkerboard mask.
--
-- The trick is DITHER_FLIP, a single global 0/1 that flips each time a new
-- stroke starts on the canvas (not on button taps). A dither stroke always
-- paints the flip's parity; painting again after a click flips it, so a
-- second dither stroke of a different colour over the same spot fills in
-- exactly the pixels the first one skipped. Two interlaced strokes together
-- are a full covering, split evenly between two colours — a pseudo-mix
-- without ever averaging colour values.
--
-- Because DITHER_FLIP changes over time, replaying a save file needs the
-- flip each stroke was drawn with, not today's value — so it's baked into
-- every stroke record alongside the color/size/points.
--
-- Controls: drag on the canvas to paint. Colour and brush buttons float in
-- front of the canvas — click one to select it. Keys 1-4 pick colours, f/d
-- pick brush kind, [ and ] change size, s saves, l loads, c clears.

CANVAS_TEX = "art_canvas"
SAVE_FILE = "art.txt"

PAPER = "FFF"
CANVAS_DIST = 13.0 -- canvas plane: pushed back, into the background
BUTTON_DIST = 3.5 -- tool buttons: floating close to the camera
FILL = 0.9 -- how much of the screen the canvas should cover once fitted
LONG_EDGE = 224 -- canvas image pixels on its longer axis

-- The four print primaries. Index order also picks keys 1-4.
COLORS = { "000", "0FF", "F0F", "FF0" }
COLOR_NAME = { "black", "cyan", "magenta", "yellow" }
-- Brush kinds. The letter is also what the save format stores.
KINDS = { "f", "d" }
KIND_NAME = { f = "full", d = "dither" }
SIZES = { 1, 3, 6, 11 }

canvas = nil -- the image we paint into
canvas_ent = nil -- the plane it's textured onto
half_w = 1.0 -- canvas half-extents in world units, set by fit_scene
half_h = 1.0
img_w = LONG_EDGE -- canvas image size, fixed once fitted
img_h = LONG_EDGE
tan_x = 0.0 -- camera FOV tangents, solved once and reused at every depth
tan_z = 0.0
fitted = false

color_i = 1
kind_i = 1
size_i = 2
strokes = {} -- fixed-width records, for saving
dither_flip = 0 -- flips on every new canvas stroke; see note at the top

color_buttons = {} -- { ent, x, z, half } in world space at BUTTON_DIST
kind_buttons = {}
color_marks = {} -- selection-highlight ents, one per button list
kind_marks = {}

drawing = false -- a stroke is in progress (started on the canvas)
was_down = false
last_x = 0 -- previous point of the stroke, in canvas pixels
last_y = 0
dirty = false -- canvas changed this frame and needs re-uploading
status = ""
status_hold = 0

-- ---------------------------------------------------------------------------
-- helpers  (identical fixed-width padding to FRESCO, for the save format)
-- ---------------------------------------------------------------------------

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
	status_hold = 150
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
-- scene geometry
--
-- Every plane the pointer can hit — canvas or a button — sits at its own
-- fixed y-distance from the camera and is measured in the same two FOV
-- tangents, solved once from a single pointer sample (see FRESCO note 1).
-- ---------------------------------------------------------------------------

function solve_fov(m)
	if m.vy < 0.2 then
		return false -- pointer is off to the side; the solve is ill-conditioned
	end
	local sx = 2.0 * m.x - 1.0
	local sy = 1.0 - 2.0 * m.y
	if abs(sx) < 0.08 or abs(sy) < 0.08 then
		return false
	end
	local tx = (m.vx / m.vy) / sx
	local tz = (m.vz / m.vy) / sy
	if tx <= 0.0 or tz <= 0.0 then
		return false
	end
	tan_x = tx
	tan_z = tz
	return true
end

-- Where the pointer ray meets the y = dist plane, in world units.
-- Returns hx, hz, or nil if the ray is parallel to the plane.
function plane_hit(m, dist)
	if m.vy < 0.001 then
		return nil, nil
	end
	local t = dist / m.vy
	return m.vx * t, m.vz * t
end

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

function solid_tex(name, col, n)
	local im = nimg(n, n)
	im:fill(col)
	tex(name, im)
end

-- 2x2 checkerboard, upscaled with nearest-style repeats, so the dither
-- button reads as "half-and-half" rather than a flat colour.
function dither_tex(name, a, b, n)
	local im = nimg(n, n)
	local x = 0
	local y = 0
	for y = 0, n - 1 do
		for x = 0, n - 1 do
			if (x + y) % 2 == 0 then
				im:pixel(x, y, a)
			else
				im:pixel(x, y, b)
			end
		end
	end
	tex(name, im)
end

function build_canvas()
	pick_image_size()
	canvas = nimg(img_w, img_h)
	canvas:fill(PAPER)
	tex(CANVAS_TEX, canvas)

	-- A thin box rather than a true plane, same trick as FRESCO: the cube mesh
	-- always exists, and a little depth reads as a physical object.
	canvas_ent = make("cube", 0., CANVAS_DIST, 0.)
	canvas_ent.tex = CANVAS_TEX
	canvas_ent.size = { half_w * 2., 0.06, half_h * 2. }
	canvas_ent.offset = { -0.5, -0.5, -0.5 }
end

-- Lay the tool buttons out along a strip near the bottom of the view, at
-- BUTTON_DIST — closer to the camera than the canvas, so they read as
-- floating in the foreground and naturally occlude it when clicked.
--
-- A table constructor that holds a local, written *inside a loop*, corrupts
-- the compiled chunk — an open silt bug (SILT-BUGS.md #5 documents the
-- `t[#t+1] = {...}` case; `ent.size = { size, 0.08, size }` inside this
-- function's own loop hits the same failure). Every such constructor below
-- is hoisted into a local first (`sz`, `rec`) and assigned from there —
-- the documented workaround — rather than written inline.
function build_buttons()
	local bw = BUTTON_DIST * tan_x * FILL
	local bh = BUTTON_DIST * tan_z * FILL
	local row_z = -bh * 0.72 -- near the bottom of the screen
	local size = bw * 0.28
	local gap = bw * 0.34
	local ncolors = #COLORS
	local nkinds = #KINDS
	local n = ncolors + nkinds
	local x0 = -((n - 1) * gap) / 2.0
	local i = 0
	local x = 0.0
	local ent = nil
	local mark = nil
	local rec = nil
	local sz = nil

	color_buttons = {}
	kind_buttons = {}
	color_marks = {}
	kind_marks = {}

	for i = 1, ncolors do
		x = x0 + (i - 1) * gap
		solid_tex("art_col" .. tostring(i), COLORS[i], 4)
		ent = make("cube", x, BUTTON_DIST, row_z)
		ent.tex = "art_col" .. tostring(i)
		sz = { size, 0.08, size }
		ent.size = sz
		ent.offset = { -0.5, -0.5, -0.5 }

		-- Selection ring: a slim cube hovering just in front, only visible
		-- (non-zero size) once this colour is picked.
		mark = make("cube", x, BUTTON_DIST - 0.22, row_z - size * 0.62)
		mark.tex = ent.tex
		mark.size = { 0., 0., 0. }
		mark.offset = { -0.5, -0.5, -0.5 }

		rec = { ent = ent, x = x, z = row_z, half = size / 2.0 }
		color_buttons[i] = rec
		color_marks[i] = mark
	end

	solid_tex("art_full", "888", 4)
	dither_tex("art_dither", "888", "DDD", 4)
	for i = 1, nkinds do
		x = x0 + (ncolors + i - 1) * gap
		ent = make("cube", x, BUTTON_DIST, row_z)
		ent.tex = (KINDS[i] == "f") and "art_full" or "art_dither"
		sz = { size, 0.08, size }
		ent.size = sz
		ent.offset = { -0.5, -0.5, -0.5 }

		mark = make("cube", x, BUTTON_DIST - 0.22, row_z - size * 0.62)
		mark.tex = ent.tex
		mark.size = { 0., 0., 0. }
		mark.offset = { -0.5, -0.5, -0.5 }

		rec = { ent = ent, x = x, z = row_z, half = size / 2.0 }
		kind_buttons[i] = rec
		kind_marks[i] = mark
	end
end

-- `sz` is hoisted rather than written as `m.size = { s, 0.05, s }` directly:
-- a table constructor holding a local, assigned inside a loop, corrupts the
-- compiled chunk (see build_buttons' note; same underlying silt issue).
function update_selection_marks()
	local i = 0
	local m = nil
	local b = nil
	local s = 0.0
	local sz = nil
	for i = 1, #color_marks do
		m = color_marks[i]
		b = color_buttons[i]
		s = (i == color_i) and (b.half * 0.8) or 0.0
		sz = { s, 0.05, s }
		m.size = sz
	end
	for i = 1, #kind_marks do
		m = kind_marks[i]
		b = kind_buttons[i]
		s = (i == kind_i) and (b.half * 0.8) or 0.0
		sz = { s, 0.05, s }
		m.size = sz
	end
end

-- Nearest-first hit test against the two floating button rows. Returns
-- "color", index or "kind", index, or nil, nil if the ray misses both.
function button_hit(m)
	local hx = 0.0
	local hz = 0.0
	local i = 0
	local b = nil
	hx, hz = plane_hit(m, BUTTON_DIST)
	if hx == nil then
		return nil, nil
	end
	for i = 1, #color_buttons do
		b = color_buttons[i]
		if abs(hx - b.x) <= b.half and abs(hz - b.z) <= b.half then
			return "color", i
		end
	end
	for i = 1, #kind_buttons do
		b = kind_buttons[i]
		if abs(hx - b.x) <= b.half and abs(hz - b.z) <= b.half then
			return "kind", i
		end
	end
	return nil, nil
end

-- Where the pointer ray meets the canvas plane, in canvas pixels. Returns
-- hx, hy, or nil, nil if the ray misses the canvas.
function canvas_hit(m)
	local hx = 0.0
	local hz = 0.0
	local u = 0.0
	local v = 0.0
	hx, hz = plane_hit(m, CANVAS_DIST)
	if hx == nil then
		return nil, nil
	end
	u = (hx + half_w) / (half_w * 2.)
	v = 1.0 - (hz + half_h) / (half_h * 2.)
	if u < 0.0 or u > 1.0 or v < 0.0 or v > 1.0 then
		return nil, nil
	end
	return u * img_w, v * img_h
end

-- ---------------------------------------------------------------------------
-- painting
--
-- Every `local` is declared at the top of its function — see FRESCO's note
-- on silt's block-scoping bug; the same rule applies here.
-- ---------------------------------------------------------------------------

-- A round brush, stamped pixel-by-pixel so the dither mask can be applied.
-- flip is passed explicitly (not read off the global) so a replayed stroke
-- always uses the flip it was originally drawn with.
function stamp(ix, iy, s, col, kind, flip)
	local r = s / 2.0
	local r2 = r * r
	local cx = flr(ix)
	local cz = flr(iy)
	local x = 0
	local y = 0
	local dx = 0
	local dy = 0
	if s <= 1 then
		if kind == "f" or (cx + cz) % 2 == flip then
			canvas:pixel(cx, cz, col)
		end
		return
	end
	for y = cz - flr(r), cz + flr(r) do
		for x = cx - flr(r), cx + flr(r) do
			dx = x - ix
			dy = y - iy
			if dx * dx + dy * dy <= r2 then
				if kind == "f" or (x + y) % 2 == flip then
					canvas:pixel(x, y, col)
				end
			end
		end
	end
end

-- Paint a segment by stamping along it, so a fast drag leaves a continuous
-- line rather than a dotted trail (the pointer only reports once a frame).
function paint_seg(x1, y1, x2, y2, col, kind, s, flip)
	local dx = x2 - x1
	local dy = y2 - y1
	local steps = flr(abs(dx))
	local sy = flr(abs(dy))
	local t = 0.0
	local i = 0
	if sy > steps then
		steps = sy
	end
	steps = flr(steps / 2)
	if steps < 1 then
		steps = 1
	end
	if steps > 180 then
		steps = 180
	end
	for i = 0, steps do
		t = i / steps
		stamp(x1 + dx * t, y1 + dy * t, s, col, kind, flip)
	end
	dirty = true
end

-- Paint with the current selection and record it, flip baked in.
function paint_and_record(x1, y1, x2, y2)
	local col = cur_color()
	local kind = cur_kind()
	local s = cur_size()
	paint_seg(x1, y1, x2, y2, col, kind, s, dither_flip)
	local rec = kind .. tostring(dither_flip) .. col .. pad2(s)
	rec = rec .. pad3(x1) .. pad3(y1) .. pad3(x2) .. pad3(y2)
	strokes[#strokes + 1] = rec
end

function clear_canvas()
	canvas:fill(PAPER)
	strokes = {}
	dirty = true
end

-- ---------------------------------------------------------------------------
-- save / load  — strokes, not pixels, same rationale as FRESCO note 2.
-- Record layout: kind(1) flip(1) color(3) size(2) x1(3) y1(3) x2(3) y2(3) = 19
-- ---------------------------------------------------------------------------

function save_drawing()
	local out = ""
	local i = 0
	for i = 1, #strokes do
		out = out .. strokes[i] .. "\n"
	end
	if io.set(SAVE_FILE, out) then
		say("saved " .. tostring(#strokes) .. " strokes")
	else
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
	local n = flr(string.len(body) / 20) -- 19 chars + newline
	local o = 1
	local kind = ""
	local flip = 0
	local col = ""
	local s = 0
	local x1 = 0
	local y1 = 0
	local x2 = 0
	local y2 = 0
	local i = 0
	for i = 0, n - 1 do
		o = i * 20 + 1
		kind = string.sub(body, o, o)
		flip = tonumber(string.sub(body, o + 1, o + 1))
		col = string.sub(body, o + 2, o + 4)
		s = tonumber(string.sub(body, o + 5, o + 6))
		x1 = tonumber(string.sub(body, o + 7, o + 9))
		y1 = tonumber(string.sub(body, o + 10, o + 12))
		x2 = tonumber(string.sub(body, o + 13, o + 15))
		y2 = tonumber(string.sub(body, o + 16, o + 18))
		if flip ~= nil and s ~= nil and x1 ~= nil and y1 ~= nil and x2 ~= nil and y2 ~= nil then
			paint_seg(x1, y1, x2, y2, col, kind, s, flip)
			strokes[#strokes + 1] = string.sub(body, o, o + 18)
		end
	end
	say("loaded " .. tostring(n) .. " strokes")
	dirty = true
end

-- ---------------------------------------------------------------------------
-- keyboard (desktop convenience; the floating buttons are the primary input)
-- ---------------------------------------------------------------------------

KEYNUM = { "1", "2", "3", "4" }
KEYKIND = { "f", "d" }

function read_keys()
	local i = 0
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
	sky:fill("456")
	cout("ART - drag the far plane to paint. floating cubes up close pick colour/brush.")
	cout("keys: 1-4 colour, f/d brush, [ ] size, s l c")
end

function loop()
	local m = mus()
	local hx = 0.0
	local hy = 0.0
	local act = nil
	local idx = 0
	cam({ pos = { 0., 0., 0. }, rot = { tau / 4, 0. } })

	-- Fit on the first frame the pointer geometry allows, then build the scene.
	if not fitted then
		if solve_fov(m) then
			half_w = CANVAS_DIST * tan_x * FILL
			half_h = CANVAS_DIST * tan_z * FILL
			build_canvas()
			build_buttons()
			update_selection_marks()
			fitted = true
			say("canvas " .. tostring(img_w) .. "x" .. tostring(img_h))
		end
		clr()
		gui:text("move the pointer to place the scene", 0.06, 0.5, "CCC")
		return
	end

	read_keys()

	local down = m.m1
	if down and not was_down then
		-- Decide once, on the press, whether this is a button tap or a stroke.
		act, idx = button_hit(m)
		if act == "color" then
			color_i = idx
			update_selection_marks()
			drawing = false
		elseif act == "kind" then
			kind_i = idx
			update_selection_marks()
			drawing = false
		else
			hx, hy = canvas_hit(m)
			if hx ~= nil then
				dither_flip = 1 - dither_flip -- new stroke: flip the interlace
				drawing = true
				last_x = hx
				last_y = hy
				paint_and_record(hx, hy, hx, hy) -- a tap leaves a dot
			end
		end
	elseif down and drawing then
		hx, hy = canvas_hit(m)
		if hx ~= nil then
			paint_and_record(last_x, last_y, hx, hy)
			last_x = hx
			last_y = hy
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
	if status_hold > 0 then
		status_hold = status_hold - 1
		gui:text(status, 0.02, 0.03, "FFE")
	end
	gui:text(COLOR_NAME[color_i] .. " / " .. KIND_NAME[cur_kind()] .. " / " .. tostring(cur_size()), 0.02, 0.94, "FFF")
end

function draw() end

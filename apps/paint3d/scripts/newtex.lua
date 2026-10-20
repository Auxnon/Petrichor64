-- NEW: give the focused thing a fresh image of its own, through a small modal.
--
-- The size starts at 16x16, or at the sheet's tile size when the focused
-- texture is a tile of a sheet (name.16.png, name.32.png). The engine paints
-- the new image to look the way the thing does now, and for a mesh lays its
-- triangles out flat on it (app.newtex: BOX, or FLAT for signs and panels).
-- Nothing is written until SAVE, which creates assets/<name>.png, or for a
-- mesh assets/<name>.glb carrying the new uvs with the image.

NT = { open = false, name = "", w = 16, h = 16, mode = "box", err = "", btn = {}, btn_n = 0 }

NT_SIZES = { 4, 8, 16, 24, 32, 48, 64, 96, 128, 256 }

function newtex_open()
	if not F.on then
		say("FOCUS SOMETHING FIRST")
		return
	end
	if F.key == "tile" then
		say("TILES SHARE THEIR IMAGES, NEW IS FOR ENTITIES")
		return
	end
	local size = 16
	local info = app.texinfo(F.tex)
	if info ~= nil and info.tile ~= nil then
		size = info.tile
	end
	NT.w = size
	NT.h = size
	NT.name = newtex_free_name(F.tex)
	NT.err = ""
	NT.open = true
end

-- The focused texture's name with _new on, numbered past any that exist.
function newtex_free_name(base)
	local b = newtex_clean(base)
	local cand = b .. "_new"
	local n = 1
	local taken = app.texinfo(cand)
	while taken ~= nil do
		n = n + 1
		local ns = tostring(n)
		cand = b .. "_new"
		cand = cand .. ns
		taken = app.texinfo(cand)
	end
	return cand
end

-- Lower case, a-z 0-9 _ only, without the trailing tile number (wall3) that
-- would be noise in a name of its own.
function newtex_clean(s)
	local low = string.lower(s)
	local len = string.len(low)
	local stop = len
	local d = string.sub(low, stop, stop)
	while stop > 1 and string.find(d, "^[0-9]$") ~= nil do
		stop = stop - 1
		d = string.sub(low, stop, stop)
	end
	local out = ""
	for i = 1, stop do
		local ch = string.sub(low, i, i)
		local ok = string.find(ch, "^[a-z0-9_]$")
		if ok == nil then
			ch = "_"
		end
		out = out .. ch
	end
	if out == "" then
		out = "image"
	end
	return out
end

function newtex_close()
	NT.open = false
end

-- One step up or down the size list from wherever the size is now.
function newtex_step(cur, dir)
	if dir > 0 then
		for i = 1, #NT_SIZES do
			local s = NT_SIZES[i]
			if s > cur then
				return s
			end
		end
		return cur
	end
	local best = cur
	for i = 1, #NT_SIZES do
		local s = NT_SIZES[i]
		if s < cur then
			best = s
		end
	end
	return best
end

function newtex_create()
	if NT.name == "" then
		NT.err = "TYPE A NAME"
		return
	end
	local r = app.newtex(F.id, NT.name, NT.w, NT.h, NT.mode)
	if not r.ok then
		local e = string.upper(r.err)
		NT.err = e
		return
	end
	NT.open = false
	-- the new image is now what this thing shows: paint it
	focus_texture(r.tex)
	mark_edited(r.tex)
	local s = "NEW "
	local t = short(r.tex, 14)
	s = s .. t
	if r.unwrapped then
		local c = tostring(r.charts)
		s = s .. ", "
		s = s .. c
		s = s .. " PIECES"
	end
	say(s)
end

-- Keys while the modal is up; true if it took the frame's keys.
function newtex_keys()
	if not NT.open then
		return false
	end
	if key("escape", true) then
		newtex_close()
		return true
	end
	if key("return", true) then
		newtex_create()
		return true
	end
	if key("back", true) then
		local len = string.len(NT.name)
		if len > 0 then
			NT.name = string.sub(NT.name, 1, len - 1)
		end
	end
	local typed = cin()
	local tn = string.len(typed)
	for i = 1, tn do
		local ch = string.sub(typed, i, i)
		ch = string.lower(ch)
		if ch == "-" or ch == " " then
			ch = "_"
		end
		local ok = string.find(ch, "^[a-z0-9_]$")
		local room = string.len(NT.name) < 24
		if ok ~= nil and room then
			NT.name = NT.name .. ch
		end
	end
	return true
end

function newtex_button(label, x, y, act, on)
	local len = string.len(label)
	local w = len * GLYPH + 8
	local h = GLYPH + 6
	local edge = "6a6f88ff"
	local bg = "151722ff"
	local fg = "c8cce0"
	if on then
		edge = FRAME_COL
		bg = "f8d040ff"
		fg = "201808"
	end
	gui:rect(x, y, w, h, edge)
	local ix = x + 1
	local iy = y + 1
	local iw = w - 2
	local ih = h - 2
	gui:rect(ix, iy, iw, ih, bg)
	local tx = x + 4
	local ty = y + 3
	gui:text(label, tx, ty, fg)
	local b = {}
	b.x = x
	b.y = y
	b.w = w
	b.h = h
	b.act = act
	NT.btn_n = NT.btn_n + 1
	NT.btn[NT.btn_n] = b
	return w
end

function newtex_draw()
	local used = 0
	local tx = 0
	NT.btn = {}
	NT.btn_n = 0
	local row = GLYPH + 8
	local lines = 6
	if NT.err ~= "" then
		lines = lines + 1
	end
	local w = 31 * GLYPH + 16
	local maxw = gui.w - 2 * INSET - 8
	if w > maxw then
		w = maxw
	end
	local h = lines * row + 16
	local x = (gui.w - w) // 2
	local y = (gui.h - h) // 2
	local sx = x + 2
	local sy = y + 2
	gui:rect(sx, sy, w, h, "0a0b12ff")
	gui:rect(x, y, w, h, "e8e4d8ff")
	local ix = x + 2
	local iy = y + 2
	local iw = w - 4
	local ih = h - 4
	gui:rect(ix, iy, iw, ih, "232637ff")

	local lx = x + 8
	local vx = x + 8 + 8 * GLYPH
	local ly = y + 8
	gui:text("NEW IMAGE", lx, ly, "f4f0e4")
	local what = short(F.tex, 14)
	local fx = x + w - 8 - string.len(what) * GLYPH
	gui:text(what, fx, ly, "6a6f88")

	-- name, with a cursor block
	ly = ly + row
	gui:text("NAME", lx, ly, "a8acc8")
	local nm = string.upper(NT.name)
	local by = ly - 3
	local bx0 = vx - 3
	local bw = x + w - 8 - bx0
	local bh = GLYPH + 6
	gui:rect(bx0, by, bw, bh, "0a0b12ff")
	gui:text(nm, vx, ly, "f8e8b0")
	-- a block cursor: an underscore one would read as part of the name
	local nlen = string.len(nm)
	local cx0 = vx + nlen * GLYPH
	gui:rect(cx0, by + 2, 6, GLYPH + 2, FRAME_COL)

	-- size
	ly = ly + row
	gui:text("SIZE", lx, ly, "a8acc8")
	local bx = vx - 3
	used = newtex_button("-", bx, ly - 3, "w-", false)
	bx = bx + used + 2
	local ws = tostring(NT.w)
	tx = bx + 2
	gui:text(ws, tx, ly, "f4f0e4")
	bx = bx + 4 * GLYPH
	used = newtex_button("+", bx, ly - 3, "w+", false)
	bx = bx + used + 8
	gui:text("X", bx, ly, "6a6f88")
	bx = bx + GLYPH + 8
	used = newtex_button("-", bx, ly - 3, "h-", false)
	bx = bx + used + 2
	local hs = tostring(NT.h)
	tx = bx + 2
	gui:text(hs, tx, ly, "f4f0e4")
	bx = bx + 4 * GLYPH
	newtex_button("+", bx, ly - 3, "h+", false)

	-- how a mesh is laid out
	ly = ly + row
	gui:text("UNWRAP", lx, ly, "a8acc8")
	local mx = vx - 3
	local boxed = NT.mode == "box"
	local flat = NT.mode == "flat"
	used = newtex_button("BOX", mx, ly - 3, "box", boxed)
	mx = mx + used + 4
	used = newtex_button("FLAT", mx, ly - 3, "flat", flat)
	mx = mx + used + 6
	gui:text("MESHES", mx, ly, "6a6f88")

	ly = ly + row
	local nmax = (w - 16) // GLYPH
	if INFO.packed then
		gui:text("PACKED GAME: LIVE ONLY, NO SAVE", lx, ly, "f08070")
	else
		gui:text("SAVES A NEW .PNG (.GLB: MESH)", lx, ly, "6a6f88")
	end
	if NT.err ~= "" then
		ly = ly + row
		local e = string.sub(NT.err, 1, nmax)
		gui:text(e, lx, ly, "f08070")
	end

	ly = ly + row
	local cx = lx - 3
	used = newtex_button("CREATE", cx, ly - 3, "create", true)
	cx = cx + used + 6
	newtex_button("CANCEL", cx, ly - 3, "cancel", false)
	local hint = "ENTER / ESC"
	local hx = x + w - 8 - string.len(hint) * GLYPH
	gui:text(hint, hx, ly, "6a6f88")
end

function newtex_click(px, py)
	for i = 1, NT.btn_n do
		local b = NT.btn[i]
		if inside(px, py, b.x, b.y, b.w, b.h) then
			newtex_act(b.act)
			return
		end
	end
end

function newtex_act(act)
	if act == "w-" then
		NT.w = newtex_step(NT.w, -1)
	elseif act == "w+" then
		NT.w = newtex_step(NT.w, 1)
	elseif act == "h-" then
		NT.h = newtex_step(NT.h, -1)
	elseif act == "h+" then
		NT.h = newtex_step(NT.h, 1)
	elseif act == "box" then
		NT.mode = "box"
	elseif act == "flat" then
		NT.mode = "flat"
	elseif act == "create" then
		newtex_create()
	elseif act == "cancel" then
		newtex_close()
	end
end

-- The overlay's chrome: a dithered frame around the screen so it's obvious the
-- app is paused under an editor, and, once something has been painted, a SAVE
-- button in the lower right that opens a confirm dialog. Saving writes the
-- edited textures back into the app's own image files (app.save); textures the
-- app drew from code have no file, and the dialog says so instead.

-- Edited textures, in the order first touched. `seen` is replaced, never
-- emptied key by key (silt: no table deletion).
E = { n = 0, names = {}, seen = {} }
-- The dialog: what can be saved (ok) and what can't (bad), and its buttons.
D = { open = false, ok = {}, ok_n = 0, bad = {}, bad_n = 0, btn = {}, btn_n = 0 }
-- The save button's rectangle, set when drawn.
SB = { on = false, x = 0, y = 0, w = 0, h = 0 }
FRAME = { w = 0, h = 0 }

INSET = 4
FRAME_COL = "f8d040ff"
GLYPH = 8

function mark_edited(name)
	if E.seen[name] then
		return
	end
	E.seen[name] = true
	E.n = E.n + 1
	E.names[E.n] = name
end

-- Ordered dots thinning out from the screen edge: solid, every other pixel,
-- every fourth, every eighth. `r` is the distance from the edge, `a` the
-- position along it.
function dither_on(a, r)
	if r == 0 then
		return true
	end
	if r == 1 then
		return a % 2 == 0
	end
	if r == 2 then
		return a % 4 == 1
	end
	return a % 8 == 3
end

function frame_build(w, h)
	FRAME.w = w
	FRAME.h = h
	local top = nimg(w, INSET)
	local bottom = nimg(w, INSET)
	local left = nimg(INSET, h)
	local right = nimg(INSET, h)
	for r = 0, INSET - 1 do
		local far = INSET - 1 - r
		for x = 0, w - 1 do
			if dither_on(x, r) then
				top:pixel(x, r, FRAME_COL)
				bottom:pixel(x, far, FRAME_COL)
			end
		end
		for y = 0, h - 1 do
			if dither_on(y, r) then
				left:pixel(r, y, FRAME_COL)
				right:pixel(far, y, FRAME_COL)
			end
		end
	end
	FRAME.top = top
	FRAME.bottom = bottom
	FRAME.left = left
	FRAME.right = right
end

-- Drawn last every frame, over whichever mode drew the rest of the gui.
function chrome_draw()
	local w = gui.w
	local h = gui.h
	if FRAME.w ~= w or FRAME.h ~= h then
		frame_build(w, h)
	end
	gui:img(FRAME.top, 0, 0)
	local by = h - INSET
	gui:img(FRAME.bottom, 0, by)
	gui:img(FRAME.left, 0, 0)
	local rx = w - INSET
	gui:img(FRAME.right, rx, 0)

	SB.on = false
	if D.open then
		dialog_draw()
	elseif E.n > 0 then
		save_button_draw()
	end
end

function save_button_draw()
	local n = tostring(E.n)
	local label = "SAVE "
	label = label .. n
	local len = string.len(label)
	local w = len * GLYPH + 8
	local h = GLYPH + 6
	SB.on = true
	SB.w = w
	SB.h = h
	SB.x = gui.w - INSET - 3 - w
	SB.y = gui.h - INSET - 3 - h
	local sx = SB.x + 1
	local sy = SB.y + 1
	gui:rect(sx, sy, w, h, "0a0b12ff")
	gui:rect(SB.x, SB.y, w, h, FRAME_COL)
	local ix = SB.x + 1
	local iy = SB.y + 1
	local iw = w - 2
	local ih = h - 2
	gui:rect(ix, iy, iw, ih, "232637ff")
	local tx = SB.x + 4
	local ty = SB.y + 3
	gui:text(label, tx, ty, "f8e8b0")
end

function inside(x, y, rx, ry, rw, rh)
	return x >= rx and y >= ry and x < rx + rw and y < ry + rh
end

-- A click in gui pixels; true if the chrome took it.
function chrome_click(x, y)
	if D.open then
		dialog_click(x, y)
		return true
	end
	if SB.on and inside(x, y, SB.x, SB.y, SB.w, SB.h) then
		dialog_open()
		return true
	end
	return false
end

-- Sort the edits into what can be written back and what can't, before asking.
function dialog_open()
	D.open = true
	D.ok = {}
	D.ok_n = 0
	D.bad = {}
	D.bad_n = 0
	for i = 1, E.n do
		local name = E.names[i]
		local r = app.source(name)
		local item = {}
		item.name = name
		if r.ok then
			item.path = r.path
			D.ok_n = D.ok_n + 1
			D.ok[D.ok_n] = item
		else
			item.err = r.err
			D.bad_n = D.bad_n + 1
			D.bad[D.bad_n] = item
		end
	end
end

function dialog_close()
	D.open = false
end

function dialog_lines()
	local lines = {}
	local n = 0
	if D.ok_n > 0 then
		local c = tostring(D.ok_n)
		local s = "SAVE "
		s = s .. c
		if D.ok_n == 1 then
			s = s .. " IMAGE FILE?"
		else
			s = s .. " IMAGE FILES?"
		end
		n = n + 1
		lines[n] = s
		local show = D.ok_n
		if show > 4 then
			show = 4
		end
		for i = 1, show do
			local it = D.ok[i]
			local p = "  "
			p = p .. it.path
			n = n + 1
			lines[n] = p
		end
		if D.ok_n > show then
			local more = tostring(D.ok_n - show)
			local m = "  +"
			m = m .. more
			m = m .. " MORE"
			n = n + 1
			lines[n] = m
		end
	else
		n = n + 1
		lines[n] = "NOTHING HERE CAN BE SAVED"
	end
	if D.bad_n > 0 then
		local c = tostring(D.bad_n)
		local s = c
		s = s .. " NOT SAVEABLE:"
		n = n + 1
		lines[n] = s
		local show = D.bad_n
		if show > 3 then
			show = 3
		end
		for i = 1, show do
			local it = D.bad[i]
			local p = "  "
			p = p .. it.name
			p = p .. " - "
			p = p .. it.err
			n = n + 1
			lines[n] = p
		end
	end
	D.lines_n = n
	return lines
end

function dialog_button(label, x, y, act)
	local len = string.len(label)
	local w = len * GLYPH + 10
	local h = GLYPH + 6
	gui:rect(x, y, w, h, FRAME_COL)
	local ix = x + 1
	local iy = y + 1
	local iw = w - 2
	local ih = h - 2
	gui:rect(ix, iy, iw, ih, "151722ff")
	local tx = x + 5
	local ty = y + 3
	gui:text(label, tx, ty, "f8e8b0")
	local b = {}
	b.x = x
	b.y = y
	b.w = w
	b.h = h
	b.act = act
	D.btn_n = D.btn_n + 1
	D.btn[D.btn_n] = b
	return w
end

function dialog_draw()
	local lines = dialog_lines()
	local n = D.lines_n
	local gw = gui.w
	local gh = gui.h
	local maxc = (gw - 2 * INSET - 24) // GLYPH
	local widest = 12
	for i = 1, n do
		local l = lines[i]
		local len = string.len(l)
		if len > widest then
			widest = len
		end
	end
	if widest > maxc then
		widest = maxc
	end
	local w = widest * GLYPH + 16
	local h = n * (GLYPH + 2) + 38
	local x = (gw - w) // 2
	local y = (gh - h) // 2

	local sx = x + 2
	local sy = y + 2
	gui:rect(sx, sy, w, h, "0a0b12ff")
	gui:rect(x, y, w, h, "e8e4d8ff")
	local ix = x + 2
	local iy = y + 2
	local iw = w - 4
	local ih = h - 4
	gui:rect(ix, iy, iw, ih, "232637ff")
	for i = 1, n do
		local l = lines[i]
		local t = string.sub(l, 1, widest)
		local ty = y + 8 + (i - 1) * (GLYPH + 2)
		local tx = x + 8
		local col = "e8e8f0"
		if string.sub(l, 1, 2) == "  " then
			col = "a8acc8"
		end
		gui:text(t, tx, ty, col)
	end

	D.btn = {}
	D.btn_n = 0
	local by = y + h - GLYPH - 14
	local bx = x + 8
	if D.ok_n > 0 then
		local used = dialog_button("SAVE", bx, by, "save")
		bx = bx + used + 6
		dialog_button("CANCEL", bx, by, "cancel")
	else
		dialog_button("OK", bx, by, "cancel")
	end
	local hint = "ENTER / ESC"
	local hx = x + w - 8 - string.len(hint) * GLYPH
	local hy = by + 3
	if hx > bx + 70 then
		gui:text(hint, hx, hy, "6a6f88")
	end
end

function dialog_click(x, y)
	for i = 1, D.btn_n do
		local b = D.btn[i]
		if inside(x, y, b.x, b.y, b.w, b.h) then
			if b.act == "save" then
				dialog_commit()
			else
				dialog_close()
			end
			return
		end
	end
end

-- Write every saveable edit. What saved, and what can never save (no file),
-- is cleared; a save that failed stays listed so it can be retried.
function dialog_commit()
	local saved = 0
	local failed = 0
	local keep = {}
	local kn = 0
	for i = 1, D.ok_n do
		local it = D.ok[i]
		local r = app.save(it.name)
		if r.ok then
			saved = saved + 1
		else
			failed = failed + 1
			kn = kn + 1
			keep[kn] = it.name
		end
	end
	E.names = keep
	E.n = kn
	E.seen = {}
	for i = 1, kn do
		local nm = keep[i]
		E.seen[nm] = true
	end
	D.open = false
	local s = "SAVED "
	local c = tostring(saved)
	s = s .. c
	if failed > 0 then
		local f = tostring(failed)
		s = s .. ", "
		s = s .. f
		s = s .. " FAILED"
	end
	say(s)
end

-- Keys while the dialog is up; true if it took the frame's keys.
function dialog_keys()
	if not D.open then
		return false
	end
	if key("escape", true) then
		dialog_close()
	elseif key("return", true) then
		if D.ok_n > 0 then
			dialog_commit()
		else
			dialog_close()
		end
	end
	return true
end

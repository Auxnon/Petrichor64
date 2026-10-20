-- The tool panel: one 128x128 image. In scene mode it's a billboard floating
-- beside the focused mesh (clicks come back from app.pick(true) as texels of
-- this image); in canvas mode the same image is drawn flat on the gui. Either
-- way a click arrives as panel pixel coordinates and goes to panel_click.

-- Buttons: { x, y, w, h, act, arg } in panel pixels.
BTN = {}
BTN_N = 0

TOOLS = { "pen", "fill", "pick" }
TOOL_LABEL = { "PEN", "FILL", "PICK" }

function btn_add(x, y, w, h, act, arg)
	local b = { x = x, y = y, w = w, h = h, act = act, arg = arg }
	BTN_N = BTN_N + 1
	BTN[BTN_N] = b
end

-- Upper-cased (the font is caps-only) and cut to n characters.
function short(s, n)
	local u = string.upper(s)
	local len = string.len(u)
	if len <= n then
		return u
	end
	local t = string.sub(u, 1, n)
	return t
end

function panel_draw()
	local im = PANEL_IMG
	BTN = {}
	BTN_N = 0
	im:clr()
	-- shadow, frame, body
	im:rrect(2, 2, 126, 126, 6, "0a0b12ff")
	im:rrect(0, 0, 126, 126, 6, "e8e4d8ff")
	im:rrect(2, 2, 122, 122, 5, "232637ff")

	local name = "NOTHING FOCUSED"
	if F.on then
		name = short(F.tex, 10)
	end
	im:text(name, 6, 6, "f4f0e4ff")
	if F.on then
		local dim = tostring(F.tw)
		dim = dim .. "X"
		local th = tostring(F.th)
		dim = dim .. th
		im:text(dim, 6, 16, "8a90b0ff")
		im_button(PANEL_IMG, 92, 12, 30, 13, "NEW")
		btn_add(92, 12, 30, 13, "new", nil)
	end

	-- tools
	for i = 1, 3 do
		local x = 5 + (i - 1) * 40
		local t = TOOLS[i]
		local bg = "3a3f58ff"
		local fg = "c8cce0ff"
		if P.tool == t then
			bg = "f8d040ff"
			fg = "201808ff"
		end
		im:rrect(x, 28, 37, 13, 3, bg)
		local label = TOOL_LABEL[i]
		local lx = x + 3
		im:text(label, lx, 31, fg)
		btn_add(x, 28, 37, 13, "tool", t)
	end

	-- brush sizes, then the current colour
	for i = 1, 3 do
		local x = 5 + (i - 1) * 17
		local bg = "3a3f58ff"
		local fg = "c8cce0ff"
		if P.size == i then
			bg = "f8d040ff"
			fg = "201808ff"
		end
		im:rrect(x, 45, 15, 13, 3, bg)
		local label = tostring(i)
		local lx = x + 4
		im:text(label, lx, 48, fg)
		btn_add(x, 45, 15, 13, "size", i)
	end
	im:rect(58, 45, 63, 13, "0a0b12ff")
	im:rect(59, 46, 61, 11, P.color)

	-- swatches: 8 per row, two rows
	for i = 1, PAL_N do
		local col = (i - 1) % 8
		local row = (i - 1) // 8
		local x = 5 + col * 15
		local y = 63 + row * 15
		local c = PAL[i]
		local edge = "0a0b12ff"
		if c == P.color then
			edge = "f8d040ff"
		end
		im:rect(x, y, 14, 14, edge)
		local ix = x + 2
		local iy = y + 2
		if c == "00000000" then
			-- the eraser swatch: a checker, since transparent shows as nothing
			im:rect(ix, iy, 10, 10, "9090a0ff")
			im:rect(ix, iy, 5, 5, "d0d0e0ff")
			local jx = ix + 5
			local jy = iy + 5
			im:rect(jx, jy, 5, 5, "d0d0e0ff")
		else
			im:rect(ix, iy, 10, 10, c)
		end
		btn_add(x, y, 14, 14, "color", c)
	end

	-- actions
	panel_button(5, 98, "UNDO", "undo")
	panel_button(45, 98, "ZOOM", "zoom")
	panel_button(85, 98, "DONE", "done")
	tex("px3_panel", im)
end

function panel_button(x, y, label, act)
	im_button(PANEL_IMG, x, y, 36, 15, label)
	btn_add(x, y, 36, 15, act, nil)
end

function im_button(im, x, y, w, h, label)
	im:rrect(x, y, w, h, 3, "4a5070ff")
	local lx = x + 3
	local ly = y + 4
	im:text(label, lx, ly, "f4f0e4ff")
end

function panel_click(px, py)
	for i = 1, BTN_N do
		local b = BTN[i]
		if px >= b.x and py >= b.y and px < b.x + b.w and py < b.y + b.h then
			panel_act(b.act, b.arg)
			return true
		end
	end
	return false
end

function panel_act(act, arg)
	if act == "tool" then
		set_tool(arg)
	elseif act == "size" then
		set_size(arg)
	elseif act == "color" then
		set_color(arg)
	elseif act == "undo" then
		undo()
	elseif act == "new" then
		newtex_open()
	elseif act == "zoom" then
		toggle_canvas()
	elseif act == "done" then
		if P.mode == "canvas" then
			toggle_canvas()
		end
		unfocus()
	end
end

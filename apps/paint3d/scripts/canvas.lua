-- Canvas mode: the focused texture blown up flat on the overlay's gui, with the
-- same panel beside it. Tools, colour and brush are the same state the 3D
-- panel uses, so nothing resets switching between the two, and the scene stays
-- visible where the gui is clear — the mesh repaints as you draw here.

C = { x = 6, y = 16, scale = 1, w = 0, h = 0, px = 0, py = 16 }

function canvas_layout()
	local gw = gui.w
	local gh = gui.h
	local avail_h = gh - 22
	local avail_w = gw - PANEL_W - 18
	local avail = avail_h
	if avail_w < avail then
		avail = avail_w
	end
	local big = F.tw
	if F.th > big then
		big = F.th
	end
	local s = avail // big
	if s < 1 then
		s = 1
	end
	C.scale = s
	C.w = F.tw * s
	C.h = F.th * s
	C.px = gw - PANEL_W - 6
	C.py = 16
end

function canvas_update(m, click)
	canvas_layout()
	local gw = gui.w
	local gh = gui.h
	local mx = flr(m.x * gw)
	local my = flr(m.y * gh)

	-- panel
	local inpx = mx - C.px
	local inpy = my - C.py
	if inpx >= 0 and inpy >= 0 and inpx < PANEL_W and inpy < PANEL_W then
		if click then
			panel_click(inpx, inpy)
		end
		canvas_draw(-1, -1)
		return
	end

	-- canvas
	local hu = -1
	local hv = -1
	local cx = mx - C.x
	local cy = my - C.y
	if cx >= 0 and cy >= 0 and cx < C.w and cy < C.h then
		hu = cx // C.scale
		hv = cy // C.scale
		local im = F.img
		local here = im:get(hu, hv)
		if click then
			stroke_begin()
			paint_at(hu, hv, here, true)
		elseif m.m1 and U.painting then
			paint_at(hu, hv, here, false)
		end
	end
	-- the panel's DONE may have dropped focus mid-frame
	if P.mode == "canvas" then
		canvas_draw(hu, hv)
	end
end

function canvas_draw(hu, hv)
	clr()
	local gw = gui.w
	gui:rect(0, 0, gw, 11, "151722ff")
	local title = short(F.tex, 24)
	local s = "ZOOM  "
	s = s .. title
	gui:text(s, 3, 2, "e8e8f0")
	local hx = gw - 84
	gui:text("ESC: BACK", hx, 2, "8a8fa8")

	-- frame, a mid grey under transparent texels, then the texture itself
	local fx = C.x - 2
	local fy = C.y - 2
	local fw = C.w + 4
	local fh = C.h + 4
	gui:rect(fx, fy, fw, fh, "0a0b12ff")
	gui:rect(C.x, C.y, C.w, C.h, "5a5a66ff")
	gui:img(F.img, C.x, C.y, C.scale)

	-- the hovered texel, outlined
	if hu >= 0 and C.scale >= 3 then
		local s2 = C.scale
		local ox = C.x + hu * s2
		local oy = C.y + hv * s2
		local ox2 = ox + s2 - 1
		local oy2 = oy + s2 - 1
		gui:rect(ox, oy, s2, 1, "ffffffff")
		gui:rect(ox, oy2, s2, 1, "ffffffff")
		gui:rect(ox, oy, 1, s2, "ffffffff")
		gui:rect(ox2, oy, 1, s2, "ffffffff")
	end

	gui:img(PANEL_IMG, C.px, C.py)
	local ty = C.py + PANEL_W + 6
	gui:text("B F I  1-3  Z", C.px, ty, "8a90b0")
	if hu >= 0 then
		local pos = tostring(hu)
		pos = pos .. ","
		local vs = tostring(hv)
		pos = pos .. vs
		local py2 = ty + 10
		gui:text(pos, C.px, py2, "c8cce0")
	end
end

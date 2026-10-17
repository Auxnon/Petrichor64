-- Scene mode: hover, focus, and painting straight onto the app's meshes.

function scene_update(m, click)
	-- The panel first: it floats in front, so it wins wherever it overlaps.
	if F.on then
		local own = app.pick(true)
		local panel = U.panel
		local pid = panel.id
		if own ~= nil and own.id == pid then
			hide(U.cursor)
			if click and own.u ~= nil then
				panel_click(own.u, own.v)
			end
			return
		end
	end

	local h = app.pick()
	cursor_update(h)
	if h == nil then
		return
	end
	if click then
		if F.on and same_target(h) then
			stroke_begin()
			paint_at(h.u, h.v, h.color, true)
		else
			focus(h)
		end
	elseif m.m1 and U.painting and same_target(h) then
		paint_at(h.u, h.v, h.color, false)
	end
end

function same_target(h)
	if h.tex == nil then
		return false
	end
	if h.tex ~= F.tex then
		return false
	end
	local k = target_key(h)
	return k == F.key
end

-- Focus whatever was clicked: remember its texture, offer the colours it was
-- painted with, and float the tool panel beside the spot.
function focus(h)
	if h.tex == nil then
		say("NO TEXTURE THERE")
		return
	end
	F.on = true
	F.key = target_key(h)
	F.tex = h.tex
	F.tw = h.tw
	F.th = h.th
	F.x = h.x
	F.y = h.y
	F.z = h.z
	F.u = h.u
	F.v = h.v
	F.img = gimg(h.tex)
	pal_from_texture(h.tex, h.color)
	if P.tool == "pick" then
		P.tool = "pen"
	end
	panel_draw()
	panel_place()
	local t = short(h.tex, 16)
	local s = "FOCUS "
	s = s .. t
	say(s)
end

function unfocus()
	F.on = false
	F.key = ""
	hide(U.panel)
	hide(U.pin)
	panel_draw()
end

-- Camera basis from app.view(): forward f, right r, up u (world units).
function cam_basis(v)
	local cy = cos(v.yaw)
	local sy = sin(v.yaw)
	local cp = cos(v.pitch)
	local sp = sin(v.pitch)
	local b = {}
	b.fx = cy * cp
	b.fy = sy * cp
	b.fz = sp
	b.rx = sy
	b.ry = -cy
	b.rz = 0
	-- u = r x f
	b.ux = b.ry * b.fz - b.rz * b.fy
	b.uy = b.rz * b.fx - b.rx * b.fz
	b.uz = b.rx * b.fy - b.ry * b.fx
	return b
end

-- Best-guess spot for the panel: beside the focused point, a little toward
-- the camera so it reads as "next to" the mesh, sized to a steady fraction of
-- the screen. It tries the roomier side first, falls back to the other, and
-- shrinks until the whole panel is on screen.
function panel_place()
	if not F.on then
		return
	end
	local v = app.view()
	if v == nil then
		return
	end
	local b = cam_basis(v)
	local dx = F.x - v.x
	local dy = F.y - v.y
	local dz = F.z - v.z
	local d = dx * b.fx + dy * b.fy + dz * b.fz
	if d < 0.5 then
		d = 0.5
	end
	local first = 1
	local sp = app.project(F.x, F.y, F.z)
	if sp ~= nil and sp.x > 0.5 then
		first = -1
	end
	-- 0.83 * depth is the screen height at that depth (45 degree fov)
	local near = d * 0.8
	local size = near * 0.83 * 0.42
	local placed = false
	local tries = 0
	while not placed and tries < 6 do
		local side = first
		if tries % 2 == 1 then
			side = -first
		end
		placed = panel_try(b, d, near, size, side)
		tries = tries + 1
		if tries % 2 == 0 then
			size = size * 0.8
		end
	end
	if not placed then
		panel_try(b, d, near, size, first)
	end
	-- the pin marks the spot the panel belongs to
	local pin = U.pin
	pin.x = F.x - b.fx * 0.02
	pin.y = F.y - b.fy * 0.02
	pin.z = F.z - b.fz * 0.02
	pin.scale = d * 0.018
end

-- Put the panel on one side; true if all of it lands on screen.
function panel_try(b, d, near, size, side)
	local off = size * 0.62 * side
	local back = d - near
	local px = F.x - b.fx * back + b.rx * off
	local py = F.y - b.fy * back + b.ry * off
	local pz = F.z - b.fz * back + b.rz * off
	local e = U.panel
	e.x = px
	e.y = py
	e.z = pz
	e.scale = size
	U.panel_side = side
	-- the quad is size wide and tall, facing the camera
	local half = size * 0.5
	local lx = px - b.rx * half - b.ux * half
	local ly = py - b.ry * half - b.uy * half
	local lz = pz - b.rz * half - b.uz * half
	local hx = px + b.rx * half + b.ux * half
	local hy = py + b.ry * half + b.uy * half
	local hz = pz + b.rz * half + b.uz * half
	local a = app.project(lx, ly, lz)
	local c = app.project(hx, hy, hz)
	if a == nil or c == nil then
		return false
	end
	local ok = true
	if a.x < 0.01 or c.x > 0.99 or a.x > 0.99 or c.x < 0.01 then
		ok = false
	end
	-- screen y grows downward, up-vector corner is the top
	if c.y < 0.05 or a.y > 0.99 then
		ok = false
	end
	return ok
end

-- A small marker on the texel under the pointer, sized to stay readable.
function cursor_update(h)
	local c = U.cursor
	if h == nil then
		hide(c)
		return
	end
	local nx = h.nx * 0.01
	local ny = h.ny * 0.01
	local nz = h.nz * 0.01
	c.x = h.x + nx
	c.y = h.y + ny
	c.z = h.z + nz
	c.scale = h.dist * 0.014
end

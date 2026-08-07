-- Chisel — a grid modeller. Place points, connect them into faces, extrude.
--
-- Runs as a normal app for now:  Petrichor64 apps/model
--
-- It is meant to become the overlay model editor (PLAN.md phase 4), but an overlay
-- with its own 3D needs the separate render pass that doesn't exist yet — without it
-- an overlay borrows the app's camera, and a modeller has to own its view.
--
-- Keys
--   arrows      move the cursor on the XY plane
--   pgup/pgdn   raise/lower the cursor a layer
--   space       place (or remove) a point at the cursor
--   enter       connect the last 3 or 4 placed points into a face
--   e           extrude the newest face one unit along its normal
--   j l i k     orbit the camera;  - =  zoom
--   x           clear everything
--   s           save to scripts/out.lua as a mod() call you can paste anywhere
--
-- Geometry is built with mod(): the quad form for the grid and the faces, which
-- takes 4 verts per quad and works out indices and UVs itself. No texture is given,
-- so everything wears the engine's fallback checker. Vectors are silt's
-- real vec3 (the `vector` feature), so normals and extrusion are cross/normalize
-- rather than hand-rolled component arithmetic.

GRID = 8 -- cells from origin to edge, so the plane is 2*GRID across
CELL = 1.0
LINE = 0.03 -- grid line half-thickness

-- The grid sits just under the working plane, and its two line directions sit on
-- slightly different levels. Coplanar quads that cross — which is every intersection
-- of a grid — fight for the same depth and shimmer; so does a face built at z=0
-- directly on top of the grid. A hair of separation is cheaper than any depth-bias
-- machinery and invisible at this scale.
GRID_Z_X = -0.02 -- lines running along X
GRID_Z_Y = -0.01 -- lines running along Y

C_BG = "112"
C_HUD = "CDE"
C_DIM = "889"
C_OK = "6D9"
C_WARN = "F96"

cursor = nil
pts = {}       -- placed points, vec3
faces = {}     -- each is an array of 3 or 4 indices into pts
recent = {}    -- indices placed since the last face, in order
mode_msg = ""
msg_left = 0

cam_az = 0.7
cam_alt = -0.5
cam_dist = 18.0

point_ents = {}
cursor_ent = nil
face_ent = nil
grid_ent = nil

function main()
	cursor = vec3(0, 0, 0)
	sky:fill("012")
	attr { title = "Chisel" }
	build_grid()
	build_cursor()
	say("place a point with space")
	cout("chisel up: arrows move, space places, enter connects, e extrudes")
end

function say(s)
	mode_msg = s
	msg_left = 300
end

-- One mesh for the whole grid: each line is a thin quad lying in the XY plane, so
-- 2*(2*GRID+1) quads go in as a single model rather than a few hundred entities.
function build_grid()
	-- table.insert rather than q[#q + 1] = {...}: a table *constructor* holding a
	-- local, assigned straight into a length-computed index, corrupts the compiled
	-- chunk in silt (see SILT-BUGS.md #5). Constants are fine; locals are not.
	local q = {}
	local n = -GRID
	while n <= GRID do
		local a = n * CELL
		local e = GRID * CELL
		-- a line along Y at x = a
		table.insert(q, { a - LINE, -e, 0 })
		table.insert(q, { a + LINE, -e, 0 })
		table.insert(q, { a + LINE, e, 0 })
		table.insert(q, { a - LINE, e, 0 })
		-- and a line along X at y = a
		table.insert(q, { -e, a - LINE, 0 })
		table.insert(q, { e, a - LINE, 0 })
		table.insert(q, { e, a + LINE, 0 })
		table.insert(q, { -e, a + LINE, 0 })
		n = n + 1
	end
	mod("chisel_grid", { q = q })
	if grid_ent == nil then
		grid_ent = make("chisel_grid", 0, 0, 0)
	end
end

function build_cursor()
	-- A small flat quad marks the cursor; it sits a hair above the grid so it is
	-- never hidden inside it.
	mod("chisel_cursor", {
		q = {
			{ -0.4, -0.4, 0.02 },
			{ 0.4,  -0.4, 0.02 },
			{ 0.4,  0.4,  0.02 },
			{ -0.4, 0.4,  0.02 },
		},
	})
	if cursor_ent == nil then
		cursor_ent = make("chisel_cursor", 0, 0, 0)
	end
end

function point_at(v)
	for i = 1, #pts do
		if pts[i] == v then
			return i
		end
	end
	return nil
end

function place()
	local hit = point_at(cursor)
	if hit ~= nil then
		-- Placing on an existing point removes it, along with any face using it.
		remove_point(hit)
		say("removed a point")
		return
	end
	pts[#pts + 1] = vec3(cursor.x, cursor.y, cursor.z)
	recent[#recent + 1] = #pts
	rebuild_points()
	say("points: " .. #pts)
end

function remove_point(idx)
	table.remove(pts, idx)
	-- Drop faces touching it, and shift the indices above it down one.
	local keep = {}
	for i = 1, #faces do
		local f = faces[i]
		local uses = false
		for j = 1, #f do
			if f[j] == idx then
				uses = true
			end
		end
		if not uses then
			local nf = {}
			for j = 1, #f do
				local v = f[j]
				if v > idx then
					v = v - 1
				end
				nf[#nf + 1] = v
			end
			keep[#keep + 1] = nf
		end
	end
	faces = keep
	recent = {}
	rebuild_points()
	rebuild_faces()
end

-- One small quad per point, all in a single model, rebuilt when the set changes.
function rebuild_points()
	for i = 1, #point_ents do
		kill(point_ents[i])
	end
	point_ents = {}
	for i = 1, #pts do
		local p = pts[i]
		point_ents[#point_ents + 1] = make("chisel_cursor", p.x, p.y, p.z + 0.03)
	end
end

function connect()
	local n = #recent
	if n < 3 then
		say("need 3 points, have " .. n)
		return
	end
	local f = {}
	local from = n - 3
	if n >= 4 then
		from = n - 4
	end
	for i = from + 1, n do
		f[#f + 1] = recent[i]
	end
	faces[#faces + 1] = f
	recent = {}
	rebuild_faces()
	say("face " .. #faces .. " of " .. #f .. " points")
end

-- The face normal, from two edges of the face. This is what extrude walks along,
-- and the reason the vector feature is worth having: cross + normalize instead of
-- nine lines of component arithmetic.
function face_normal(f)
	local a = pts[f[1]]
	local b = pts[f[2]]
	local c = pts[f[3]]
	local n = (b - a):cross(c - a)
	if n:length() < 0.0001 then
		return vec3(0, 0, 1)
	end
	return n:normalize()
end

function extrude()
	if #faces < 1 then
		say("nothing to extrude")
		return
	end
	local f = faces[#faces]
	local dir = face_normal(f) * CELL

	-- The new cap: every point of the face, moved along the normal.
	local cap = {}
	for i = 1, #f do
		local moved = pts[f[i]] + dir
		local existing = point_at(moved)
		if existing == nil then
			pts[#pts + 1] = moved
			cap[#cap + 1] = #pts
		else
			cap[#cap + 1] = existing
		end
	end

	-- A wall per edge, joining the old ring to the new one.
	for i = 1, #f do
		local j = i + 1
		if j > #f then
			j = 1
		end
		table.insert(faces, { f[i], f[j], cap[j], cap[i] })
	end
	faces[#faces + 1] = cap

	rebuild_points()
	rebuild_faces()
	say("extruded; faces: " .. #faces)
end

-- Faces go in as one model. Triangles are padded to a quad by repeating the last
-- point, because the quad form wants groups of four and a degenerate edge costs
-- nothing to draw.
function rebuild_faces()
	if #faces < 1 then
		if face_ent ~= nil then
			kill(face_ent)
			face_ent = nil
		end
		return
	end
	local q = {}
	for i = 1, #faces do
		local f = faces[i]
		for j = 1, 4 do
			local k = j
			if k > #f then
				k = #f
			end
			local p = pts[f[k]]
			table.insert(q, { p.x, p.y, p.z })
		end
	end
	mod("chisel_faces", { q = q })
	if face_ent == nil then
		face_ent = make("chisel_faces", 0, 0, 0)
	end
end

function save()
	-- Emit the mesh as a mod() call: the same form this app builds with, so the
	-- output can be pasted into any app or loaded back here.
	local out = "-- built with chisel\nmod(\"mymodel\", { q = {\n"
	for i = 1, #faces do
		local f = faces[i]
		for j = 1, 4 do
			local k = j
			if k > #f then
				k = #f
			end
			local p = pts[f[k]]
			out = out .. "\t{ " .. p.x
			out = out .. ", " .. p.y
			out = out .. ", " .. p.z
			out = out .. " },\n"
		end
	end
	out = out .. "} })\n"
	if io.set("scripts/out.lua", out) then
		say("saved " .. #faces .. " faces to scripts/out.lua")
	else
		say("save refused")
	end
end

function clear_all()
	pts = {}
	faces = {}
	recent = {}
	rebuild_points()
	rebuild_faces()
	say("cleared")
end

function move_cursor(dx, dy, dz)
	local x = cursor.x + dx * CELL
	local y = cursor.y + dy * CELL
	local z = cursor.z + dz * CELL
	local lim = GRID * CELL
	if x > lim then x = lim end
	if x < -lim then x = -lim end
	if y > lim then y = lim end
	if y < -lim then y = -lim end
	cursor = vec3(x, y, z)
end

function loop()
	if msg_left > 0 then
		msg_left = msg_left - 1
	end

	if key("left", true) then move_cursor(-1, 0, 0) end
	if key("right", true) then move_cursor(1, 0, 0) end
	if key("up", true) then move_cursor(0, 1, 0) end
	if key("down", true) then move_cursor(0, -1, 0) end
	if key("pageup", true) then move_cursor(0, 0, 1) end
	if key("pagedown", true) then move_cursor(0, 0, -1) end

	if key("space", true) then place() end
	if key("enter", true) then connect() end
	if key("e", true) then extrude() end
	if key("x", true) then clear_all() end
	if key("s", true) then save() end

	if key("j") then cam_az = cam_az - 0.03 end
	if key("l") then cam_az = cam_az + 0.03 end
	if key("i") then cam_alt = cam_alt - 0.02 end
	if key("k") then cam_alt = cam_alt + 0.02 end
	if key("-") then cam_dist = cam_dist + 0.2 end
	if key("=") then cam_dist = cam_dist - 0.2 end
	if cam_dist < 4.0 then cam_dist = 4.0 end
	if cam_dist > 40.0 then cam_dist = 40.0 end

	-- Orbit: spherical offset from the cursor, so the view follows what you edit.
	local flat = cos(cam_alt) * cam_dist
	local eye = vec3(
		cursor.x + cos(cam_az) * flat,
		cursor.y + sin(cam_az) * flat,
		cursor.z - sin(cam_alt) * cam_dist
	)
	cam { pos = { eye.x, eye.y, eye.z }, rot = { cam_az + pi, cam_alt } }

	if cursor_ent ~= nil then
		cursor_ent.x = cursor.x
		cursor_ent.y = cursor.y
		cursor_ent.z = cursor.z
	end

	hud()
end

function hud()
	clr()
	rect(0, 0, 320, 10, "223")
	local head = "CHISEL  pts " .. #pts
	head = head .. "  faces " .. #faces
	text(head, 2, 1, C_HUD)

	local at = "cursor " .. flr(cursor.x)
	at = at .. ","
	at = at .. flr(cursor.y)
	at = at .. ","
	at = at .. flr(cursor.z)
	text(at, 2, 12, C_DIM)

	local sel = "selected " .. #recent
	text(sel, 2, 22, C_DIM)

	rect(0, 220, 320, 20, "223")
	text("space place  enter face  e extrude  s save", 2, 222, C_DIM)
	text("arrows move  pgup/dn layer  jlik orbit", 2, 231, C_DIM)

	if msg_left > 0 then
		text(mode_msg, 2, 32, C_OK)
	end
end

function draw()
end

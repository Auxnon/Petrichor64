t = {
	sidewall = 11,
	topwall = 12,
	sidedoor = 13,
	floor = 14,
	grass1 = 15,
	grass2 = 26,
	grass3 = 25,
	treeside = 16,
	treetop = 17,
	trunk = 18,
}



sky()
-- "cadc9f"
-- "9bbc0f"
-- "306230"
-- "b8d9b8"


fill("9bbc0f") --cadc9f
gui()

dimg(gimg("speech"), .5, 50)
function main()
	for key, val in pairs(t) do
		t[key] = "bip" .. val
		-- print(t[key])
	end
	cube("leaves", t.treetop, t.treeside, t.treeside, t.treeside, t.treeside, t.treeside)
	t.leaves = "leaves"
	cube("wall", t.topwall, t.sidewall, t.sidewall, t.sidewall, t.sidewall, t.sidewall)
	t.wall = "wall"
	cube("door", t.topwall, t.sidedoor, t.sidewall, t.sidedoor, t.sidewall, t.sidewall)
	t.door = "door"
	feller = spawn('wall', 4, 0, 1)
	feller2 = spawn('bip3', 4, 0, 1)
	if true then


		for i = -6, 6 do
			for j = -6, 6 do
				tile(t["grass" .. irnd(1, 4)], i, j, 0)
			end
		end

		-- tile(t.floor, 0, 2, 0)
		-- tile(t.floor, 1, 2, 0)
		for i = -2, 2 do
			for j = -2, 2 do
				tile(t.floor, i, j, 0)
			end
		end

		for i = -2, 2 do
			tile(t.wall, 3, i, 1)
		end

		for i = -2, 2 do
			tile(t.wall, -3, i, 1)
		end

		for i = -3, 3 do
			tile(t.wall, i, 3, 1)
		end
		for i = -3, 3 do
			tile(t.wall, i, -3, 1)
		end

		tile(t.door, 0, 3, 1, 1)
		tile(t.door, 0, -3, 1, 1)

		tile(t.leaves, 4, 0, 2)
		tile(t.leaves, 5, 0, 2, 1)
		tile(t.leaves, 5, 1, 2, 2)
		tile(t.leaves, 4, 1, 2, 3)
	else
		tile(t.trunk, -3, 0, 0)
		-- tile(t.trunk, 0, 8, 0)
		-- tile(t.trunk, 0, 0, 8)
		-- print(t.trunk)
		-- for i = -2, 2 do
		--     for j = -2, 2 do
		--         tile(t.trunk, i, j, -1)
		--     end
		-- end

	end


	-- tile(t.grass, 4, 12, 0)
	cam { pos = { 0, -8, 8 }, rot = { pi / 2, -tau / 8 } }

end

rr = 0.
last_pos = { 0, 0, 0 }
first_click = true
remove_mode = false
cpos = { 0, -8, 8 }
function loop()
	-- example.x = example.x + rnd() * 0.1 - 0.05
	-- example.z = example.z + rnd() * 0.1 - 0.05
	local m = mouse()
	-- local x = flr((m.x * 16) - 8)
	-- local y = flr((16 - m.y * 16) - 8)

	local vv = 18.
	local vx = m.vx * vv + cpos[1]
	local vy = m.vy * vv + cpos[2]
	local vz = m.vz * vv + cpos[3]

	local f = -(cpos[3]) / (m.vz)
	local p = { x = cpos[1] + f * m.vx, y = cpos[2] + f * m.vy, z = 1 + cpos[3] + f * m.vz }
	-- print(x, y, z)
	feller2.x = vx
	feller2.y = vy
	feller2.z = vz
	local xx = flr(p.x + .5)
	local yy = flr(p.y + .5)
	local zz = flr(p.z + .5)

	feller.x = xx - .5
	feller.y = yy - .5
	feller.z = zz - .5
	if m.m1 then


		if first_click then
			first_click = false
			-- print("" .. is_tile(xx, yy, 1))
			if is_tile(xx, yy, 1) then
				remove_mode = true
				print "remove"
			else
				remove_mode = false
			end
		end

		-- print(xx, yy)

		-- local x=m.x*16
		if xx == last_pos[1] and yy == last_pos[2] then
			return
		else
			last_pos[1] = xx
			last_pos[2] = xx
			if remove_mode then
				tile(0, xx, yy, 1)
			else
				tile(t.wall, xx, yy, 1)
			end
		end
	else
		first_click = true
	end
	cam { pos = cpos, rot = { rr + pi / 2, -tau / 8 } }
	-- cam { pos = cpos, rot = { 0, tau / 2 } }
	if key("a") then
		rr = rr + 0.02
	elseif key("d") then
		rr = rr - 0.02
	end
	if key("w") then
		cpos[1] = cpos[1] - sin(rr) * 0.2
		cpos[2] = cpos[2] + cos(rr) * 0.2
	elseif key("s") then
		cpos[1] = cpos[1] + sin(rr) * 0.2
		cpos[2] = cpos[2] - cos(rr) * 0.2
	end
	if key("space") then
		cpos[1] = 0
		cpos[2] = -8
		cpos[3] = 8
		rr = 0
	end

end

-- Rig — a live showroom for the graphics "chip" and display "monitor" presets.
--
-- Two textured cubes sit side by side: the left one carries the current chip
-- (R00/R43/R30), the right one the current monitor (LCD/Slot/Grille). Cycling
-- either calls chip()/mon() live, so the actual render pipeline changes on the
-- test-pattern texture in real time — wobble/affine warp and dithering for R30,
-- soft blur for R43, and the CRT post-pass look for Slot/Grille.
--
-- Keys
--   left/right   cycle the chip
--   up/down      cycle the monitor
--   j l i k      orbit the camera;  - =  zoom

CHIPS = {
	{ code = "r00", name = "R00", desc = "modern: full precision, perspective-correct" },
	{ code = "r43", name = "R43", desc = "N64-ish: soft bilinear blur, lower res" },
	{ code = "r30", name = "R30", desc = "PS1-ish: vertex wobble, affine warp, dithered" },
}
MONITORS = {
	{ code = "lcd", name = "LCD", desc = "modern flat panel, no CRT artifacts" },
	{ code = "slot", name = "Slot", desc = "slot-mask CRT: soft consumer look" },
	{ code = "grille", name = "Grille", desc = "aperture-grille CRT: crisp, punchy" },
}

chip_i = 1
mon_i = 1

C_BG = "112"
C_HUD = "CDE"
C_DIM = "889"

cam_az = 0.6
cam_alt = -0.35
cam_dist = 9.0

chip_ent = nil
mon_ent = nil

function main()
	sky:fill("134")
	attr { title = "Rig" }
	mod("rig_box", { t = { "swatch", "swatch", "swatch", "swatch", "swatch", "swatch" } })
	chip_ent = make("rig_box", -1.4, 0, 0)
	mon_ent = make("rig_box", 1.4, 0, 0)
	apply_chip()
	apply_mon()
	cout("rig up: left/right cycles chip, up/down cycles monitor")
end

function apply_chip()
	chip(CHIPS[chip_i].code)
end

function apply_mon()
	mon(MONITORS[mon_i].code)
end

function loop()
	if key("left", true) then
		chip_i = chip_i - 1
		if chip_i < 1 then chip_i = #CHIPS end
		apply_chip()
	end
	if key("right", true) then
		chip_i = chip_i + 1
		if chip_i > #CHIPS then chip_i = 1 end
		apply_chip()
	end
	if key("up", true) then
		mon_i = mon_i - 1
		if mon_i < 1 then mon_i = #MONITORS end
		apply_mon()
	end
	if key("down", true) then
		mon_i = mon_i + 1
		if mon_i > #MONITORS then mon_i = 1 end
		apply_mon()
	end

	if key("j") then cam_az = cam_az - 0.03 end
	if key("l") then cam_az = cam_az + 0.03 end
	if key("i") then cam_alt = cam_alt - 0.02 end
	if key("k") then cam_alt = cam_alt + 0.02 end
	if key("-") then cam_dist = cam_dist + 0.2 end
	if key("=") then cam_dist = cam_dist - 0.2 end
	if cam_dist < 3.0 then cam_dist = 3.0 end
	if cam_dist > 24.0 then cam_dist = 24.0 end

	-- Orbit around the origin, midway between the two cubes.
	local flat = cos(cam_alt) * cam_dist
	local eye = vec3(cos(cam_az) * flat, sin(cam_az) * flat, -sin(cam_alt) * cam_dist)
	cam { pos = { eye.x, eye.y, eye.z }, rot = { cam_az + pi, cam_alt } }

	-- A slow spin keeps the affine warp/wobble visibly moving rather than a
	-- static frame, which is easy to mistake for a still image.
	if chip_ent ~= nil then
		chip_ent.rot_z = chip_ent.rot_z + 0.01
	end
	if mon_ent ~= nil then
		mon_ent.rot_z = mon_ent.rot_z + 0.01
	end

	hud()
end

function hud()
	clr()
	rect(0, 0, 320, 22, "223")
	text("RIG  chip " .. CHIPS[chip_i].name .. "  monitor " .. MONITORS[mon_i].name, 2, 2, C_HUD)
	text(CHIPS[chip_i].desc, 2, 12, C_DIM)

	rect(0, 220, 320, 20, "223")
	text(MONITORS[mon_i].desc, 2, 222, C_DIM)
	text("left/right chip  up/down monitor  jlik orbit  -/= zoom", 2, 231, C_DIM)
end

function draw()
end

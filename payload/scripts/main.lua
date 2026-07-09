-- Codex 3.0.0 "Artichoke"
sky:fill("FF5")

function main()
	local im = nimg(16, 16)
	im:fill("00F")
	im:line(0, 0, 10, 10, "F0F")
	tex("generated", im)
	example = make("example", 12, rnd() * 3. - 1.5, rnd() * 3. - 1.5)
	generated = make("generated", 10, rnd() * 3. - 1.5, rnd() * 3. - 1.5)
	spin = 0

	for i = 0, 6, 1 do
		for j = 0, 6, 1 do
			local t = "example"
			if (j + i + 1) % 2 == 0 then
				t = "generated"
			end
			tile(t, 9 + j, i - 3, -3)
		end
	end

	-- Capture the mouse for FPS-style look. On web the pointer locks on the
	-- next canvas click; Esc frees it (re-locks on the next click).
	mgrab(true)
	yaw = 0
	pitch = -tau / 16
	cout("main runs once everything has loaded")
end

function loop()
	example.y = example.y + rnd(-0.05, 0.05)
	example.z = example.z + rnd(-0.05, 0.05)
	spin = spin + 0.1
	generated.z = generated.z + cos(spin) * 0.04
	generated.y = generated.y + sin(spin) * 0.04

	-- Mouse-look: accumulate yaw/pitch from relative movement (dx/dy).
	local m = mus()
	yaw = yaw - m.dx * 0.005
	pitch = pitch - m.dy * 0.005
	if pitch > 1.5 then pitch = 1.5 end
	if pitch < -1.5 then pitch = -1.5 end
	cam({ pos = { 0, 0, 0 }, rot = { yaw, pitch } })
end

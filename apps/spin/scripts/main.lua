-- Spin — a single cube rotating on all three axes at different rates.
-- Minimal geometry sanity check for the render-tui backend (src/tui/renderer.rs):
-- confirms per-entity model + rotation composition (not just a static marker).
--
-- Run with: just tui apps/spin
--   (or: cargo run --no-default-features --features silt,render-tui -- apps/spin)

function main()
	fill("fff")
	cube = make("example", 0, 0, 0, 1)
	cube.scale = 2
	-- cube.tex="

	-- Camera sits on -X looking toward +X (rot={0,0} => look direction is
	-- +X, see src/tui/renderer.rs::camera_matrix), so the cube at the
	-- origin sits dead ahead.
	cam({ pos = { -8, 0, 0 }, rot = { 0, 0 } })
end

function loop()
	-- cube.x=cube.x+.1
	-- cube.rx = cube.rx + 0.010
	-- cube.ry = cube.ry + 0.023
	if key("a") then
		cube.rz += 0.07
	elseif key("d") then
		cube.rz -= 0.07
	end

	if key("w") then
		cube.z += 0.07
	elseif key("s") then
		cube.z -= 0.07
	end
end

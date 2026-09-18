-- guys — a small character-controller demo.
--
-- One player-controlled "guy" and ten NPC "guys" (same asset, same base
-- class — see guy.lua) walk around a generated hill. Everyone falls off
-- tiles they aren't standing on, auto-steps 1-tile-high ledges, and is
-- blocked by anything 2 tiles tall. A 3x3 floating tile cluster sits past a
-- 1-wide chasm at hill-peak height — reachable only by jumping.
--
-- World building and the Guy/Player/Npc classes live in world.lua/guy.lua;
-- everything here only calls into them from inside main()/loop()/draw(), so
-- the load order of the three files (not guaranteed by the engine) doesn't
-- matter — nothing at top level in any file depends on another file's
-- globals already existing.
--
-- Axis convention: Z-up (see world.lua) — x,y are the ground plane, z is
-- height, so the spawn ring and the trailing camera both work in x/y with a
-- z offset rather than the x/z-plane-with-y-height layout a Y-up engine
-- would use.

player = nil
npcs = {}

NPC_COUNT = 10

function spawn_npcs(n)
	local i = 0
	local ang = 0.0
	local x = 0.0
	local y = 0.0
	for i = 1, n do
		ang = (i / n) * tau
		x = 2.0 + cos(ang) * 3.0
		y = sin(ang) * 3.0
		local tint = { r = 0.55 + (i % 3) * 0.12, g = 0.5, b = 0.9 - (i % 4) * 0.12, a = 1.0 }
		npcs[i] = Npc.new(x, y, 1.0, tint)
	end
end

function main()
	sky:fill("79C")
	build_world()
	player = Player.new(0.0, 0.0, 1.0)
	spawn_npcs(NPC_COUNT)
	cout("guys: WASD/dpad move, space/south jump, camera follows")
end

function loop()
	player:update_input()
	player:update_physics()

	local i = 0
	for i = 1, #npcs do
		npcs[i]:update_ai(player, npcs)
	end
	for i = 1, #npcs do
		npcs[i]:update_physics()
	end

	local pent = player.ent
	local pos = { pent.x - 6.0, pent.y, pent.z + 4.0 }
	local rot = { -0.5, 0.0 }
	cam({ pos = pos, rot = rot })

	clr()
	gui:text("guys demo", 0.02, 0.03, "FFF")
	gui:text("wasd/dpad move  space/south jump", 0.02, 0.94, "DEF")
end

function draw() end

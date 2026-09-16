-- guys — world terrain.
--
-- Builds a flat plain, a stepped hill (each ring exactly 1 tile taller so it
-- is auto-jump-climbable), a 1-wide chasm, and a 3x3 floating tile cluster on
-- the far side of the chasm at hill-peak height, reachable only by jumping.
--
-- Axis convention: this engine is Z-up (see amb_sky's hemisphere ambient,
-- which mixes by the surface normal's z component) — x,y are the ground
-- plane, z is height. WORLD.height is a flat "x,y" -> top-surface-z lookup,
-- built alongside the real tiles as we place them. It exists purely so Lua
-- movement/AI code can ask "how tall is this column" in O(1) instead of
-- scanning gtile() — the engine has no such query native (see guide/hit.md).
-- Actual physical collision still goes through the real hit.cell() system in
-- guy.lua; this table is a planning aid, not the source of truth for physics.

WORLD = { height = {} }

function world_key(x, y)
	return tostring(x) .. "," .. tostring(y)
end

function world_set_height(x, y, h)
	WORLD.height[world_key(x, y)] = h
end

function world_height_at(x, y)
	return WORLD.height[world_key(x, y)]
end

GROUND_MIN = -14
GROUND_MAX = 14

CHASM_X = 10
CHASM_Y0 = -1
CHASM_Y1 = 1

HILL_X0 = 3
HILL_X1 = CHASM_X - 1
HILL_Y0 = -1
HILL_Y1 = 1

CLUSTER_X0 = CHASM_X + 1
CLUSTER_X1 = CHASM_X + 3
CLUSTER_Y0 = -1
CLUSTER_Y1 = 1
CLUSTER_Z = 4

function build_ground()
	local te = nimg(1, 1)
	te:fill('ff0')
	tex('ground', te)
	local x = 0
	local y = 0
	for x = GROUND_MIN, GROUND_MAX do
		for y = GROUND_MIN, GROUND_MAX do
			if not (x == CHASM_X and y >= CHASM_Y0 and y <= CHASM_Y1) then
				tile('ground', x, y, 0, 0)
				world_set_height(x, y, 1)
			end
		end
	end
end

-- One step of height every two columns of x, so any two adjacent columns
-- differ by at most 1 tile — always auto-step climbable, never a 2-high wall.
function build_hill()
	local x = 0
	local y = 0
	local h = 0
	local step = 0
	for x = HILL_X0, HILL_X1 do
		step = flr((x - HILL_X0) / 2) + 1
		for y = HILL_Y0, HILL_Y1 do
			for h = 1, step do
				tile('cube', x, y, h, 0)
			end
			world_set_height(x, y, step + 1)
		end
	end
end

function build_cluster()
	local x = 0
	local y = 0
	for x = CLUSTER_X0, CLUSTER_X1 do
		for y = CLUSTER_Y0, CLUSTER_Y1 do
			tile('cube', x, y, CLUSTER_Z, 0)
			world_set_height(x, y, CLUSTER_Z + 1)
		end
	end
end

function build_world()
	build_ground()
	build_hill()
	build_cluster()
end

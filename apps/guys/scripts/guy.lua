-- guys — the shared Guy behavior, and its Player / Npc specializations.
--
-- Player and Npc both wrap a "guy" entity and share Guy's movement/gravity/
-- collision code; only input (Player) vs. follow-AI (Npc) differs. Uses
-- asset "guy", which deliberately does not exist in apps/guys/assets — a
-- live check that missing textures fall back cleanly instead of crashing.
--
-- Axis convention: Z-up (see world.lua) — x,y are the ground plane, z is
-- height. Gravity/jump act on vz and ent.z; dx/dy are horizontal deltas.

MOVE_SPEED = 0.09
NPC_SPEED_SCALE = 0.8
GRAVITY = 0.02
MAX_FALL = 0.6
JUMP_VELOCITY = 0.34
FALL_RESPAWN_Z = -20

-- table constructors holding a local, written as the RHS of a field/index
-- assignment inside a loop, corrupt the compiled chunk (SILT-BUGS.md #5).
-- Every table literal below that ends up on a field/index LHS is instead
-- built into a plain `local` first, then assigned from that local.

Guy = {}
Guy.__index = Guy

function Guy.new(x, y, z, tint)
	local ent = make('guy', x, y, z, 1.0)
	ent.hit_shape = 2 -- cylinder
	local sz = { 0.3, 0.3, 0.5 }
	ent.hit_size = sz
	local off = { 0, 0, 0.5 }
	ent.hit_offset = off
	if tint ~= nil then
		ent.tint = tint
	end

	local g = setmetatable({}, Guy)
	g.ent = ent
	g.vz = 0.0
	g.grounded = false
	local sp = { x = x, y = y, z = z }
	g.spawn = sp
	return g
end

function Guy:foot_height()
	local ent = self.ent
	return world_height_at(flr(ent.x + 0.5), flr(ent.y + 0.5))
end

-- Moves by (dx,dy) if the destination column isn't more than 1 tile taller
-- than the current one (a 1-tile step auto-climbs; 2+ tiles blocks outright).
-- A destination with no recorded height at all is open air/a gap: walking
-- there is always allowed (that's how a guy walks off an edge and falls).
function Guy:try_move(dx, dy)
	local ent = self.ent
	if dx == 0.0 and dy == 0.0 then
		return
	end
	local tx = flr(ent.x + dx + 0.5)
	local ty = flr(ent.y + dy + 0.5)
	local cur_h = self:foot_height()
	local tgt_h = world_height_at(tx, ty)
	local cur = cur_h or ent.z

	if tgt_h ~= nil and (tgt_h - cur) > 1.001 then
		return
	end

	ent.x = ent.x + dx
	ent.y = ent.y + dy
	if self.grounded and tgt_h ~= nil and tgt_h > cur then
		ent.z = tgt_h -- auto-jump: snap up the 1-tile step
	end
end

function Guy:jump()
	if self.grounded then
		self.vz = JUMP_VELOCITY
		self.grounded = false
	end
end

function Guy:respawn()
	local ent = self.ent
	ent.x = self.spawn.x
	ent.y = self.spawn.y
	ent.z = self.spawn.z
	self.vz = 0.0
	self.grounded = false
end

-- Gravity + the heightmap ground check, then a hit.cell() pass against the
-- real static-world collider as a correcting backstop — this is the part
-- that actually exercises the engine's tile collision system, not just our
-- own heightmap bookkeeping.
function Guy:update_physics()
	local ent = self.ent

	self.vz = self.vz - GRAVITY
	if self.vz < -MAX_FALL then
		self.vz = -MAX_FALL
	end
	ent.z = ent.z + self.vz

	local h = self:foot_height()
	if h ~= nil and ent.z <= h and self.vz <= 0.0 then
		ent.z = h
		self.vz = 0.0
		self.grounded = true
	else
		self.grounded = false
	end

	local hits = hit.cell(ent.id)
	local i = 0
	local hh = nil
	for i = 1, #hits do
		hh = hits[i]
		ent.x = ent.x + hh.normal[1] * hh.depth
		ent.y = ent.y + hh.normal[2] * hh.depth
		ent.z = ent.z + hh.normal[3] * hh.depth
		if hh.normal[3] > 0.5 then
			self.grounded = true
			if self.vz < 0.0 then
				self.vz = 0.0
			end
		end
	end

	if ent.z < FALL_RESPAWN_Z then
		self:respawn()
	end
end

-- Player: WASD or dpad to move, space or south-button to jump.

Player = setmetatable({}, { __index = Guy })
Player.__index = Player

function Player.new(x, y, z)
	local g = Guy.new(x, y, z, nil)
	local p = setmetatable(g, Player)
	p.prev_south = false
	return p
end

function Player:update_input()
	local ix = 0.0
	local iy = 0.0
	if key('w') or btn('dup') then
		iy = iy - 1.0
	end
	if key('s') or btn('ddown') then
		iy = iy + 1.0
	end
	if key('a') or btn('dleft') then
		ix = ix - 1.0
	end
	if key('d') or btn('dright') then
		ix = ix + 1.0
	end

	local len = sqrt(ix * ix + iy * iy)
	if len > 0.0001 then
		ix = ix / len * MOVE_SPEED
		iy = iy / len * MOVE_SPEED
		self:try_move(ix, iy)
	end

	local south = btn('south')
	if key('space', true) or (south and not self.prev_south) then
		self:jump()
	end
	self.prev_south = south
end

-- Npc: follows the player at a distance of 1-2 tiles, avoids crowding other
-- Npcs, auto-jumps 1-high steps like anyone else, and will attempt a 1-tile
-- gap jump only if it can see solid ground on the far side within reach.

Npc = setmetatable({}, { __index = Guy })
Npc.__index = Npc

FOLLOW_MIN = 1.5
FOLLOW_MAX = 2.5
CROWD_DIST = 1.2
CROWD_WEIGHT = 0.6

function Npc.new(x, y, z, tint)
	local g = Guy.new(x, y, z, tint)
	return setmetatable(g, Npc)
end

-- Returns (can_go, needs_jump) for a proposed horizontal step (dx,dy).
-- A step onto a column with a known height is always fine (try_move already
-- blocks a 2+-tile wall). A step into a gap (no recorded height) is only
-- taken if there is a landing column within jump range and it isn't an
-- upward jump of more than 1 tile — otherwise the Npc decides not to try.
function Npc:can_go(dx, dy)
	local ent = self.ent
	local tx = flr(ent.x + dx + 0.5)
	local ty = flr(ent.y + dy + 0.5)
	local tgt_h = world_height_at(tx, ty)
	if tgt_h ~= nil then
		return true, false
	end

	local jx = flr(ent.x + dx * 2.0 + 0.5)
	local jy = flr(ent.y + dy * 2.0 + 0.5)
	local land_h = world_height_at(jx, jy)
	local cur = self:foot_height() or ent.z
	if land_h ~= nil and (land_h - cur) <= 1.001 then
		return true, true
	end
	return false, false
end

function Npc:update_ai(player, others)
	local ent = self.ent
	local pent = player.ent
	local dx = pent.x - ent.x
	local dy = pent.y - ent.y
	local dist = sqrt(dx * dx + dy * dy)

	local mx = 0.0
	local my = 0.0
	if dist > FOLLOW_MAX then
		mx = dx / dist
		my = dy / dist
	elseif dist < FOLLOW_MIN and dist > 0.0001 then
		mx = -dx / dist
		my = -dy / dist
	end

	local i = 0
	local other = nil
	local oent = nil
	local ox = 0.0
	local oy = 0.0
	local od = 0.0
	local sx = 0.0
	local sy = 0.0
	for i = 1, #others do
		other = others[i]
		if other ~= self then
			oent = other.ent
			ox = ent.x - oent.x
			oy = ent.y - oent.y
			od = sqrt(ox * ox + oy * oy)
			if od < CROWD_DIST and od > 0.0001 then
				sx = sx + ox / od
				sy = sy + oy / od
			end
		end
	end
	mx = mx + sx * CROWD_WEIGHT
	my = my + sy * CROWD_WEIGHT

	local len = sqrt(mx * mx + my * my)
	if len > 0.0001 then
		mx = mx / len * MOVE_SPEED * NPC_SPEED_SCALE
		my = my / len * MOVE_SPEED * NPC_SPEED_SCALE

		local can_go, needs_jump = self:can_go(mx, my)
		if can_go then
			if needs_jump and self.grounded then
				self:jump()
			end
			self:try_move(mx, my)
		end
	end
end

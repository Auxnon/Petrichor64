-- skin: skeletal animation demo (guide/bone.md).
--
-- player.glb is a Blender rig with three single-frame clips: idle, sit and
-- carry. Left/right switches clip; the head bone is turned by script on top
-- of whatever clip is playing, and a marker cube rides the right hand.

CLIPS = { "idle", "sit", "carry" }
clip_i = 1
t = 0

function main()
	guy = make("player", 0, 0, 0, 1)
	marker = make("cube", 0, 0, 0, 0.15)
	local names = guy.bones
	cout("skin: " .. tostring(#names) .. " bones, first " .. names[1])
	guy:anim(CLIPS[clip_i])
	cam({ pos = { -5, 0, 2 }, rot = { 0, -0.1 } })
end

function loop()
	t = t + 1
	if key("right", true) or t % 120 == 60 then
		clip_i = clip_i % #CLIPS + 1
		guy:anim(CLIPS[clip_i])
		local hand = guy:bone("hand.r")
		cout("clip " .. CLIPS[clip_i] .. " hand.r z=" .. tostring(hand.z))
	end
	-- look around: turn the head on top of the clip
	local turn = { 0, sin(t * 0.05) * 0.6, 0 }
	guy:bone("head", turn)
	local h = guy:bone("hand.r")
	marker.x = h.x
	marker.y = h.y
	marker.z = h.z
	if t == 30 then
		local head = guy:bone("head")
		cout("head override ry=" .. tostring(head.ry) .. " missing=" .. tostring(guy:bone("nope")))
	end
end

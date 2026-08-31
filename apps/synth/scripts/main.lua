-- Codex 3.0.0 "Artichoke"
-- A tiny music keyboard: number keys 1..0 play a C-major scale (C4..E5).
-- Press several at once for a chord (each note takes its own voice).
-- The white keys are 3D boxes that dip while their key is held.
sky:fill("114")
r = 0

-- Globals (scale/whites/white_x) are shared across main() and loop(); constants
-- are inlined so nothing relies on chunk-level upvalue capture.
scale = {
	{ "1", 261.63 },
	{ "2", 293.66 },
	{ "3", 329.63 },
	{ "4", 349.23 },
	{ "5", 392.00 },
	{ "6", 440.00 },
	{ "7", 493.88 },
	{ "8", 523.25 },
	{ "9", 587.33 },
	{ "0", 659.25 },
}
chorus = {
	{ "q", "la la laa", { 261.63, 329.63, 392.00 }, 0.4 },
	{ "w", "sa ta sha", 440, 0.4 },
	{ "e", "do re mi", { { 261, 0.3 }, { 293, 0.3 }, { 329, 0.6 } } },
	{ "r", "ai", 430 },
	{ "t", "ai", 60 },
	{ "y", "la ma ra ra", 60 },
	-- {'y','la',220},
	-- {'u','sa',220},
	{ "u", "do re mi", { { 261, 0.3 }, { 293, 0.3 }, { 329, 0.6 } } },
	{ "i", "ta", 220 },
	{ "o", "sha", 220 },
	{ "p", "ka", 220 },
}
whites = {}
-- Which white key the pointer was on last frame (0 = none), for press-edge
-- detection the same way `key(name, true)` works for the keyboard.
touch_key = 0

function white_x(i) -- i = 1..10, centred around 0 (0.75 leaves a gap between keys)
	return (i - 5.5) * 0.75
end

-- Which white key the pointer is over, or 0 for none.
--
-- `mus()` hands back vx,vy,vz: the cursor unprojected into a normalised direction
-- from the camera. Intersect that ray with the plane of the key tops and see which
-- key's footprint the hit lands in — the flat-plane case is all this needs, since
-- every key top sits at the same height. One code path serves a mouse on desktop
-- and a finger on a phone, because touch drives mus() too.
function key_under_pointer(m)
	if not m.m1 then
		return 0
	end
	-- The camera looks down at the keys, so a ray that isn't descending can't reach
	-- them (and would divide by ~0 below).
	if m.vz > -0.0001 then
		return 0
	end
	local t = (0.2 - 9.0) / m.vz -- camera z = 9 down to the key tops at z = 0.2
	local hx = 0.0 + m.vx * t -- camera x = 0
	local hy = 3.0 + m.vy * t -- camera y = 3
	if hy < 3.0 or hy > 5.0 then -- keys are 2 deep, centred at y = 4
		return 0
	end
	for i = 1, #scale do
		if abs(hx - white_x(i)) <= 0.21 then -- half of size.x
			return i
		end
	end
	return 0
end

-- A flat colour with a faint darker speckle, so adjacent keys read as distinct
-- surfaces instead of one solid slab.
function noisy(size, base, speck, chance)
	local im = nimg(size, size)
	im:fill(base)
	for y = 0, size - 1 do
		for x = 0, size - 1 do
			if rnd() < chance then
				im:pixel(x, y, speck)
			end
		end
	end
	return im
end

-- Bake a short "plucked string" one-shot: a few harmonics that fade to silence
-- over the length of the buffer. `smpl` then pitches this around the keyboard.
function pluck_sample(f, n)
	local sr = 44100 -- author at 44.1kHz (a note at `f` plays the buffer ~untouched)
	local buf = {}
	for i = 1, n do
		local t = (i - 1) / sr
		local p = (i - 1) / n -- 0..1 through the buffer
		-- Gentle decay that keeps energy up front then rings out (reads louder
		-- and fuller than a straight linear fade).
		local decay = (1 - p) * (1 - p)
		local s = sin(t * f * tau) * 0.6 + sin(t * f * 2 * tau) * 0.3 + sin(t * f * 3 * tau) * 0.15
		buf[i] = s * decay
	end
	return buf
end

function main()
	mute()

	instr(1, { 1, 0, 0.5, 0, 0.3 }) -- additive organ-ish tone (instrument 1)
	-- Instrument 2 = a sound loaded from disk: sounds/tone.ogg (a 440Hz pluck,
	-- made with `oggify`). The keys pitch it around its 440 base. This is the
	-- retro sampler reading a real file — the string lookup happens once here.
	-- cfg gives it an ADSR envelope: a soft edge in and a gentle tail out.
	smpl(2, "tone", { atk = 0.005, rel = 0.15 })
	-- grit(2,.7,'hard')
	-- crsh(2,6,8000)
	-- filt(2,'low',3000)
	-- fade(1,2)

	-- vox{vib=.04, breath=3}
	-- echo(0,.5,.9)
	-- filt(0,'high',400,6)
	-- filt(0,'notch',200,8)
	-- filt(0,'low',200,8)
	-- vox{breath=.15, vib=.03}
	-- vox{breath=.4, vib=.05, hz=6}
	-- vox{breath=0, vib=0}

	-- Alternatively, synthesize the sample in Lua (no file needed):
	--   smpl(2, pluck_sample(220, 17000), 220)

	-- Solid-ish key surfaces. `make('cube')` resolves to the cube *mesh* (a real
	-- rectangular prism once scaled); a bare texture name would instead resolve
	-- to a flat camera-facing sprite. `e.tex` overlays the colour onto the mesh.
	tex("kw", noisy(16, "EEE", "CCC", 0.30)) -- ivory-white with light speckle
	tex("kb", noisy(16, "111", "000", 0.35)) -- near-black with darker speckle

	-- White keys: one per playable note, laid out left-to-right at depth y=4.
	for i = 1, #scale do
		local e = make("cube", white_x(i), 4.0, 0.0)
		e.tex = "kw"
		e.size = { 0.42, 2.0, 0.4 } -- width (x), depth (y), height (z); < spacing => gap
		e.offset = { -0.5, -0.5, -0.5 } -- the cube mesh is corner-anchored; centre it
		whites[i] = e
	end

	-- Black keys: decorative, between the white pairs that have a sharp between
	-- them (C-D, D-E, F-G, G-A, A-B, ...). Raised (higher z) and set back.
	local black_after = { 1, 2, 4, 5, 6, 8, 9 }
	for _, p in ipairs(black_after) do
		local bx = (white_x(p) + white_x(p + 1)) / 2
		local e = make("cube", bx, 3.65, 0.45)
		e.tex = "kb"
		e.size = { 0.26, 1.2, 0.5 }
		e.offset = { -0.5, -0.5, -0.5 }
	end

	-- Raised, angled look down onto the keyboard.
	cam({ pos = { 0, 3, 9 }, rot = { tau / 4, tau * (-1 / 5) } })
	-- Overhead sun + hemisphere ambient (cool sky above, dark bounce below) so
	-- key tops read cooler/brighter than their shaded sides and undersides.
	lamp({
		dir = { -0.35, 0.25, -0.9 },
		color = "fe8", -- warm sun
		sky = "8ad", -- cool sky ambient on the tops
		ground = "210", -- dark bounce underneath
	})
	-- A touch of distance fog fading toward the sky colour.
	fog({ color = "113", dist = 220 })
	cout("piano: press number keys 1-0 (chords work); keys dip while held")
end

function loop()
	if key("z", true) then
		crsh(2, 4, 600)
	end

	local hit = key_under_pointer(mus())

	for i = 1, #scale do
		local kname = scale[i][1]
		local e = whites[i]
		-- Ease the key toward its target height (down while held, up otherwise).
		local target = 0.0
		if key(kname) or hit == i then
			target = -0.14
		end
		e.z += (target - e.z) * 0.4
		-- Trigger the note once, on the first frame of the press (instrument 1).
		-- Touch: fire when the pointer *arrives* on a key, so holding still plays
		-- once but sliding across the keyboard glissandos, like a real one.
		if hit == i and touch_key ~= i then
			note(scale[i][2], 0.5, 2, 2)
		end
		if key(kname, true) then
			note(scale[i][2], 0.5, 2, 2)
		-- `, true` = fire once on the press edge; plain key() is held (true every
		-- frame) and would retrigger sing() ~60x/s, overlapping each syllable's
		-- vowel with the next one's consonant.
		elseif key(chorus[i][1], true) then
			local n = chorus[i]
			sing(n[2], n[3], n[4] or 1)
		end
	end
	cam({ pos = { 0, 3, 9 }, rot = { tau / 4, tau * (r - 1 / 5) } })
	r = 0.01
	touch_key = hit
end

function draw() end

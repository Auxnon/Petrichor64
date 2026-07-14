-- Codex 3.0.0 "Artichoke"
-- A tiny music keyboard: number keys 1..0 play a C-major scale (C4..E5).
-- Press several at once for a chord (each note takes its own voice).
-- The white keys are 3D boxes that dip while their key is held.
sky:fill('114')
r=0

-- Globals (scale/whites/white_x) are shared across main() and loop(); constants
-- are inlined so nothing relies on chunk-level upvalue capture.
scale = {
	{ '1', 261.63 }, { '2', 293.66 }, { '3', 329.63 }, { '4', 349.23 }, { '5', 392.00 },
	{ '6', 440.00 }, { '7', 493.88 }, { '8', 523.25 }, { '9', 587.33 }, { '0', 659.25 }
}
whites = {}

function white_x(i) -- i = 1..10, centred around 0 (0.75 leaves a gap between keys)
	return (i - 5.5) * 0.75
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

function main()
	mute()
	instr(1, { 1, 0, .5, 0, .3 }) -- additive organ-ish tone for the keys

	-- Solid-ish key surfaces. `make('cube')` resolves to the cube *mesh* (a real
	-- rectangular prism once scaled); a bare texture name would instead resolve
	-- to a flat camera-facing sprite. `e.tex` overlays the colour onto the mesh.
	tex('kw', noisy(16, 'EEE', 'CCC', 0.30)) -- ivory-white with light speckle
	tex('kb', noisy(16, '111', '000', 0.35)) -- near-black with darker speckle

	-- White keys: one per playable note, laid out left-to-right at depth y=4.
	for i = 1, #scale do
		local e = make('cube', white_x(i), 4.0, 0.0)
		e.tex = 'kw'
		e.size = { 0.42, 2.0, 0.4 } -- width (x), depth (y), height (z); < spacing => gap
		e.offset = { -0.5, -0.5, -0.5 } -- the cube mesh is corner-anchored; centre it
		whites[i] = e
	end

	-- Black keys: decorative, between the white pairs that have a sharp between
	-- them (C-D, D-E, F-G, G-A, A-B, ...). Raised (higher z) and set back.
	local black_after = { 1, 2, 4, 5, 6, 8, 9 }
	for _, p in ipairs(black_after) do
		local bx = (white_x(p) + white_x(p + 1)) / 2
		local e = make('cube', bx, 3.65, 0.45)
		e.tex = 'kb'
		e.size = { 0.26, 1.2, 0.5 }
		e.offset = { -0.5, -0.5, -0.5 }
	end

	-- Raised, angled look down onto the keyboard.
	cam { pos = { 0, 3, 9 }, rot = { tau / 4,  tau*(-1 / 5) } }
	-- Overhead sun + hemisphere ambient (cool sky above, dark bounce below) so
	-- key tops read cooler/brighter than their shaded sides and undersides.
	lamp {
		dir = { -0.35, 0.25, -0.9 },
		color = 'fe8', -- warm sun
		sky = '8ad', -- cool sky ambient on the tops
		ground = '210', -- dark bounce underneath
	}
	-- A touch of distance fog fading toward the sky colour.
	fog { color = '113', dist = 220 }
	cout('piano: press number keys 1-0 (chords work); keys dip while held')
end

function loop()
	for i = 1, #scale do
		local kname = scale[i][1]
		local e = whites[i]
		-- Ease the key toward its target height (down while held, up otherwise).
		local target = 0.0
		if key(kname) then
			target = -0.14
		end
		e.z += (target - e.z) * .4
		-- Trigger the note once, on the first frame of the press (instrument 1).
		if key(kname, true) then
			note(scale[i][2], 0.5, nil, 1)
		end
	end
	cam { pos = { 0, 3, 9 }, rot = { tau / 4,  tau*(r -1 / 5) } }
    r=0.01
end

function draw() end

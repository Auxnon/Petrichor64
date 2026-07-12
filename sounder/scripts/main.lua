-- Codex 3.0.0 "Artichoke"
-- A tiny music keyboard: number keys 1..0 play a C-major scale (C4..E5).
-- Press several at once for a chord (each note takes its own voice).
-- The white keys are 3D boxes that dip while their key is held.
sky:fill('114')

-- Globals (scale/whites/white_x) are shared across main() and loop(); constants
-- are inlined so nothing relies on chunk-level upvalue capture.
scale = {
	{ '1', 261.63 }, { '2', 293.66 }, { '3', 329.63 }, { '4', 349.23 }, { '5', 392.00 },
	{ '6', 440.00 }, { '7', 493.88 }, { '8', 523.25 }, { '9', 587.33 }, { '0', 659.25 }
}
whites = {}

function white_x(i) -- i = 1..10, centred around 0
	return (i - 5.5) * 0.62
end

function main()
	mute()

	-- Solid-colour textures. An asset name that isn't a model falls back to the
	-- cube mesh with that texture, so these become white / black key boxes.
	local w = nimg(4, 4)
	w:fill('FFF')
	tex('kw', w)
	local b = nimg(4, 4)
	b:fill('111')
	tex('kb', b)

	-- White keys: one per playable note, laid out left-to-right at depth y=4.
	for i = 1, #scale do
		local e = make('kw', white_x(i), 4.0, 0.0)
		e.size = { 0.5, 2.0, 0.4 } -- width (x), depth (y), height (z)
		whites[i] = e
	end

	-- Black keys: decorative, between the white pairs that have a sharp between
	-- them (C-D, D-E, F-G, G-A, A-B, ...). Raised (higher z) and set back.
	local black_after = { 1, 2, 4, 5, 6, 8, 9 }
	for _, p in ipairs(black_after) do
		local bx = (white_x(p) + white_x(p + 1)) / 2
		local e = make('kb', bx, 3.65, 0.45)
		e.size = { 0.32, 1.2, 0.5 }
	end

	-- Level look along +Y so the row of keys sits centred in view.
	cam { pos = { 0, -10, 10 }, rot = { tau / 4, -tau/9} }
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
		e.z = e.z + (target - e.z) * 0.4
		-- Trigger the note once, on the first frame of the press.
		if key(kname, true) then
			note(scale[i][2], 0.5)
		end
	end
end

function draw() end

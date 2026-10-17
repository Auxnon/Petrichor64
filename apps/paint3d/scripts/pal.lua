-- Palette and the overlay's own textures.
--
-- Colours are kept as 8-digit rrggbbaa strings throughout: that's what
-- app.pick, app.colors and im:get return, so comparing them (the fill tool,
-- the selected swatch) is plain string equality.

PAL = {}
PAL_N = 0
PAL_MAX = 16

-- A small general palette, used to top up whatever the texture offers.
BASE = {
	"000000ff", "ffffffff", "7f7f7fff", "c3c3c3ff",
	"e04040ff", "f0a030ff", "f8e060ff", "60c050ff",
	"40a0e0ff", "4050c0ff", "a060d0ff", "f080b0ff",
	"704020ff", "c08050ff", "203040ff", "00000000",
}

function pal_reset()
	PAL = {}
	PAL_N = 0
	for i = 1, #BASE do
		local c = BASE[i]
		pal_add(c)
	end
end

function pal_add(c)
	if PAL_N >= PAL_MAX then
		return
	end
	for i = 1, PAL_N do
		if PAL[i] == c then
			return
		end
	end
	PAL_N = PAL_N + 1
	PAL[PAL_N] = c
end

-- Best guess at the colours the texture was painted with: the colour that was
-- clicked, then the texture's most common colours, then the base palette to
-- fill the remaining swatches.
function pal_from_texture(name, clicked)
	PAL = {}
	PAL_N = 0
	if clicked ~= nil then
		pal_add(clicked)
	end
	local cols = app.colors(name)
	local n = #cols
	if n > 11 then
		n = 11
	end
	for i = 1, n do
		local c = cols[i]
		pal_add(c)
	end
	for i = 1, #BASE do
		local c = BASE[i]
		pal_add(c)
	end
	-- start on the texture's main colour if the current one isn't on offer
	local found = false
	for i = 1, PAL_N do
		if PAL[i] == P.color then
			found = true
		end
	end
	if not found and clicked ~= nil then
		P.color = clicked
	end
end

PANEL_W = 128

function textures_init()
	PANEL_IMG = nimg(PANEL_W, PANEL_W)
	tex("px3_panel", PANEL_IMG)

	-- pointer marker: a hollow square, dark rim inside a light one
	local cur = nimg(8, 8)
	cur:rect(0, 0, 8, 8, "ffffffff")
	cur:rect(1, 1, 6, 6, "101018ff")
	cur:rect(2, 2, 4, 4, "00000000")
	tex("px3_cur", cur)

	-- focus pin: a yellow diamond-ish dot
	local pin = nimg(8, 8)
	pin:rect(2, 0, 4, 8, "f8d040ff")
	pin:rect(0, 2, 8, 4, "f8d040ff")
	pin:rect(2, 2, 4, 4, "201808ff")
	tex("px3_pin", pin)
end

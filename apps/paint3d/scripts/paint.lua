-- The tools. Every edit goes into F.img (a copy of the focused texture from
-- gimg) and is pushed back with tex(), which lands it in the atlas and so on
-- every mesh that uses that texture, live.

UNDO_MAX = 40

-- A new stroke: snapshot for undo, then start painting.
function stroke_begin()
	U.painting = true
	U.lastu = -1
	U.lastv = -1
	if P.tool ~= "pick" then
		undo_push()
	end
end

-- Apply the current tool at texel (u, v). `color` is what's there now (for
-- the colour picker); `first` is the stroke's first texel.
function paint_at(u, v, color, first)
	if u == nil or v == nil then
		return
	end
	if P.tool == "pick" then
		if color ~= nil then
			set_color(color)
		end
		return
	end
	if P.tool == "fill" then
		if first then
			flood(u, v)
			commit()
		end
		return
	end
	if U.lastu < 0 then
		stamp(u, v)
	else
		stamp_line(U.lastu, U.lastv, u, v)
	end
	U.lastu = u
	U.lastv = v
	commit()
end

function commit()
	tex(F.tex, F.img)
	mark_edited(F.tex)
end

-- A square brush of P.size texels, centred on (u, v).
function stamp(u, v)
	local n = P.size
	local half = (n - 1) // 2
	local x = u - half
	local y = v - half
	local im = F.img
	if n == 1 then
		if u >= 0 and v >= 0 and u < F.tw and v < F.th then
			im:pixel(u, v, P.color)
		end
		return
	end
	-- a negative integer means "from the far edge" to the image API, so clip
	local w = n
	local h = n
	if x < 0 then
		w = w + x
		x = 0
	end
	if y < 0 then
		h = h + y
		y = 0
	end
	if w > 0 and h > 0 then
		im:rect(x, y, w, h, P.color)
	end
end

-- Stamp every texel between two points, so a quick drag leaves no gaps.
function stamp_line(u0, v0, u1, v1)
	local du = u1 - u0
	local dv = v1 - v0
	local adu = du
	if adu < 0 then
		adu = -adu
	end
	local adv = dv
	if adv < 0 then
		adv = -adv
	end
	local steps = adu
	if adv > steps then
		steps = adv
	end
	if steps == 0 then
		stamp(u1, v1)
		return
	end
	for i = 1, steps do
		local fu = du * i / steps
		local fv = dv * i / steps
		local x = flr(fu + 0.5)
		local y = flr(fv + 0.5)
		x = u0 + x
		y = v0 + y
		stamp(x, y)
	end
end

-- 4-way flood fill of the region under (sx, sy). The stack is two arrays and a
-- counter (silt can't delete from tables, and doesn't need to here).
function flood(sx, sy)
	local im = F.img
	local w = F.tw
	local h = F.th
	local target = im:get(sx, sy)
	local c = P.color
	if target == nil or target == c then
		return
	end
	local XS = {}
	local YS = {}
	local seen = {}
	local n = 1
	XS[1] = sx
	YS[1] = sy
	local painted = 0
	while n > 0 do
		local x = XS[n]
		local y = YS[n]
		n = n - 1
		local key = y * w + x
		if seen[key] ~= true then
			seen[key] = true
			local cur = im:get(x, y)
			if cur == target then
				im:pixel(x, y, c)
				painted = painted + 1
				if x > 0 then
					n = n + 1
					XS[n] = x - 1
					YS[n] = y
				end
				if x < w - 1 then
					n = n + 1
					XS[n] = x + 1
					YS[n] = y
				end
				if y > 0 then
					n = n + 1
					XS[n] = x
					YS[n] = y - 1
				end
				if y < h - 1 then
					n = n + 1
					XS[n] = x
					YS[n] = y + 1
				end
			end
		end
		if painted > 262144 then
			break
		end
	end
end

function undo_push()
	if F.img == nil then
		return
	end
	if H.n >= UNDO_MAX then
		undo_trim()
	end
	local cur = F.img
	local snap = cur:copy()
	H.n = H.n + 1
	H.img[H.n] = snap
	H.tex[H.n] = F.tex
end

-- Drop the oldest half by rebuilding the arrays (silt can't delete entries).
function undo_trim()
	local keep = UNDO_MAX // 2
	local start = H.n - keep + 1
	local imgs = {}
	local names = {}
	local j = 0
	for i = start, H.n do
		j = j + 1
		local a = H.img[i]
		local b = H.tex[i]
		imgs[j] = a
		names[j] = b
	end
	H.img = imgs
	H.tex = names
	H.n = j
end

function undo()
	if H.n < 1 then
		say("NOTHING TO UNDO")
		return
	end
	local snap = H.img[H.n]
	local name = H.tex[H.n]
	H.n = H.n - 1
	tex(name, snap)
	mark_edited(name)
	if F.on and F.tex == name then
		F.img = snap:copy()
	end
	say("UNDO")
end

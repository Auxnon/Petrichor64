-- Loom — the source editor, as an overlay.
--
-- It edits the app *underneath* it, not itself:
--
--     overlay apps/edit        (from the engine console, while a game runs)
--     overlay off
--
-- Only the engine can summon an overlay, and the privileged `app.*` table exists
-- only in an overlay's VM, so a game can neither open this nor reach another app's
-- files through it. See PLAN.md's overlay trust boundary.
--
-- Keys
--   browse: up/down pick, enter opens, f5 re-lists
--   edit:   arrows/pgup/pgdn move, ctrl+left|right jump to line ends, end = line end
--           enter splits (keeping indent), back/del, tab inserts a tab
--           click to place the cursor, ctrl+s writes and reloads the app, esc = browse
--
-- Layout is a character grid: glyphs are 8x8 with 2px of leading (LETTER_SIZE in
-- gui.rs), and the gui raster boots at 320x240 — 40 columns by 24 rows. Grid
-- positions are passed as *integers* so they're read as pixels; a float would be
-- taken for a percentage (see the gunit guide), hence flr() on every coordinate.
-- The backdrop alone is a percentage rect, so it still covers a resized raster.

CW = 8
CH = 10
COLS = 40
ROWS = 24
GUTTER = 4 -- "123 "
BODY_TOP = 1
BODY_ROWS = 21
STATUS_ROW = 23
TEXT_COLS = COLS - GUTTER
TAB_W = 4

C_BG = "112"
C_BAR = "334"
C_TEXT = "DDE"
C_NUM = "667"
C_NUM_ON = "BBC"
C_KEY = "7CF"
C_STR = "DA7"
C_COM = "587"
C_CURSOR = "FE0"
C_PICK = "5CF"
C_DIRTY = "F76"
C_HINT = "889"

KEYWORDS = { "function", "local", "end", "if", "then", "else", "elseif", "for",
	"while", "do", "return", "nil", "true", "false", "not", "and", "or", "in",
	"repeat", "until", "break" }

-- Held-key repeat, so moving and deleting don't need one press per character.
REPEAT_DELAY = 20
REPEAT_RATE = 3
held = nil
held_for = 0

mode = "browse"
files = {}
pick = 1
name = nil
lines = { "" }
cy = 1
cx = 1
top = 1
left = 0
touched = false
msg = ""
msg_left = 0
blink = 0
redraw = true

SHIFT = {}

function init_shift()
	-- cin() reports unshifted characters only, so the shifted row is ours to map.
	SHIFT["1"] = "!"
	SHIFT["2"] = "@"
	SHIFT["3"] = "#"
	SHIFT["4"] = "$"
	SHIFT["5"] = "%"
	SHIFT["6"] = "^"
	SHIFT["7"] = "&"
	SHIFT["8"] = "*"
	SHIFT["9"] = "("
	SHIFT["0"] = ")"
	SHIFT["-"] = "_"
	SHIFT["="] = "+"
	SHIFT["["] = "{"
	SHIFT["]"] = "}"
	SHIFT["\\"] = "|"
	SHIFT[";"] = ":"
	SHIFT["'"] = "\""
	SHIFT[","] = "<"
	SHIFT["."] = ">"
	SHIFT["/"] = "?"
	SHIFT["`"] = "~"
end

function shift_str(s)
	local out = ""
	for i = 1, #s do
		local c = s:sub(i, i)
		local m = SHIFT[c]
		if m == nil then
			out = out .. c:upper()
		else
			out = out .. m
		end
	end
	return out
end

function main()
	init_shift()
	load_files()
	cout("edit overlay up: up/down + enter to open, ctrl+s saves and reloads")
end

function msg_set(s)
	msg = s
	msg_left = 240
	redraw = true
end

function load_files()
	files = app.list()
	if pick > #files then
		pick = #files
	end
	if pick < 1 then
		pick = 1
	end
	redraw = true
	if #files < 1 then
		msg_set("the app underneath has no files")
	end
end

-- Split on newlines by hand: silt has find/match/gsub but no gmatch.
function split(text)
	local out = {}
	local at = 1
	while true do
		local nl = text:find("\n", at, true)
		if nl == nil then
			out[#out + 1] = text:sub(at)
			return out
		end
		out[#out + 1] = text:sub(at, nl - 1)
		at = nl + 1
	end
end

function open_file(path)
	local src = app.read(path)
	if src == nil then
		msg_set("cannot read " .. path)
		return
	end
	name = path
	lines = split(src)
	if #lines < 1 then
		lines = { "" }
	end
	cy = 1
	cx = 1
	top = 1
	left = 0
	touched = false
	mode = "edit"
	msg_set("opened " .. path)
end

function save()
	if name == nil then
		return
	end
	local body = table.concat(lines, "\n")
	if app.write(name, body) then
		touched = false
		app.reload()
		msg_set("wrote + reloaded " .. name)
	else
		msg_set("write refused: " .. name)
	end
end

-- Tabs are kept in the file and expanded only for display. Fixed width rather than
-- real tab stops, so this and disp_col can't disagree about where a glyph lands.
function expand(s)
	local out = s:gsub("\t", "    ")
	return out
end

function disp_col(s, col)
	local d = 0
	for i = 1, col - 1 do
		if s:sub(i, i) == "\t" then
			d = d + TAB_W
		else
			d = d + 1
		end
	end
	return d
end

-- The line buffer is rebuilt rather than shifted in place. On a table grown with
-- t[#t+1] — which is what split() produces — silt's table.insert(t, pos, v)
-- overwrites position `pos` instead of shifting (losing a line, with # unchanged),
-- and table.remove leaves a nil hole behind. test/silt-callarg carries the case.
function ins_at(t, at, v)
	local out = {}
	for i = 1, #t do
		if i == at then
			out[#out + 1] = v
		end
		out[#out + 1] = t[i]
	end
	if at > #t then
		out[#out + 1] = v
	end
	return out
end

function del_at(t, at)
	local out = {}
	for i = 1, #t do
		if i ~= at then
			out[#out + 1] = t[i]
		end
	end
	return out
end

function insert(s)
	local l = lines[cy]
	lines[cy] = l:sub(1, cx - 1) .. s .. l:sub(cx)
	cx = cx + #s
	touched = true
	scroll_to_cursor()
	redraw = true
end

function backspace()
	if cx > 1 then
		local l = lines[cy]
		lines[cy] = l:sub(1, cx - 2) .. l:sub(cx)
		cx = cx - 1
	elseif cy > 1 then
		local prev = lines[cy - 1]
		cx = #prev + 1
		lines[cy - 1] = prev .. lines[cy]
		lines = del_at(lines, cy)
		cy = cy - 1
	else
		return
	end
	touched = true
	scroll_to_cursor()
	redraw = true
end

function del_fwd()
	local l = lines[cy]
	if cx <= #l then
		lines[cy] = l:sub(1, cx - 1) .. l:sub(cx + 1)
	elseif cy < #lines then
		lines[cy] = l .. lines[cy + 1]
		lines = del_at(lines, cy + 1)
	else
		return
	end
	touched = true
	redraw = true
end

function newline()
	local l = lines[cy]
	local head = l:sub(1, cx - 1)
	local tail = l:sub(cx)
	local indent = head:match("^[ \t]*")
	if indent == nil then
		indent = ""
	end
	lines[cy] = head
	lines = ins_at(lines, cy + 1, indent .. tail)
	cy = cy + 1
	cx = #indent + 1
	touched = true
	scroll_to_cursor()
	redraw = true
end

function move(dx, dy)
	if dy ~= 0 then
		cy = cy + dy
		if cy < 1 then
			cy = 1
		end
		if cy > #lines then
			cy = #lines
		end
		if cx > #lines[cy] + 1 then
			cx = #lines[cy] + 1
		end
	end
	if dx ~= 0 then
		cx = cx + dx
		if cx < 1 then
			if cy > 1 then
				cy = cy - 1
				cx = #lines[cy] + 1
			else
				cx = 1
			end
		elseif cx > #lines[cy] + 1 then
			if cy < #lines then
				cy = cy + 1
				cx = 1
			else
				cx = #lines[cy] + 1
			end
		end
	end
	scroll_to_cursor()
	redraw = true
end

function scroll_to_cursor()
	if cy < top then
		top = cy
	end
	if cy > top + BODY_ROWS - 1 then
		top = cy - BODY_ROWS + 1
	end
	if top < 1 then
		top = 1
	end
	local d = disp_col(lines[cy], cx)
	if d < left then
		left = d
	end
	if d > left + TEXT_COLS - 1 then
		left = d - TEXT_COLS + 1
	end
	if left < 0 then
		left = 0
	end
end

-- True on the initial press and again on repeat while the key stays down.
function hit(k)
	if key(k, true) then
		held = k
		held_for = 0
		return true
	end
	if held == k then
		if key(k) then
			held_for = held_for + 1
			if held_for > REPEAT_DELAY then
				if ((held_for - REPEAT_DELAY) % REPEAT_RATE) == 0 then
					return true
				end
			end
			return false
		end
		held = nil
	end
	return false
end

function loop()
	blink = blink + 1
	if msg_left > 0 then
		msg_left = msg_left - 1
		if msg_left == 0 then
			redraw = true
		end
	end

	if mode == "browse" then
		browse_input()
	else
		edit_input()
	end

	-- Redraw on change, plus a slow tick so the cursor can blink.
	if redraw or (blink % 15) == 0 then
		render()
		redraw = false
	end
end

function browse_input()
	if hit("down") then
		pick = pick + 1
		redraw = true
	end
	if hit("up") then
		pick = pick - 1
		redraw = true
	end
	if pick > #files then
		pick = #files
	end
	if pick < 1 then
		pick = 1
	end
	if key("enter", true) then
		if files[pick] ~= nil then
			open_file(files[pick])
		end
	end
	if key("f5", true) then
		load_files()
		msg_set("re-listed")
	end

	local m = mus()
	if m.m1 then
		local row = flr(m.y * ROWS) - BODY_TOP + 1
		if row >= 1 and row <= #files then
			pick = row
			redraw = true
		end
	end
end

function edit_input()
	local ctrl = key("lctrl") or key("rctrl")
	local shift = key("lshift") or key("rshift")

	if ctrl then
		-- cin() reports the character whether or not ctrl is down, so chorded keys
		-- have to be handled before typing or ctrl+s would also insert an "s".
		if key("s", true) then
			save()
		end
		if key("left", true) then
			cx = 1
			scroll_to_cursor()
			redraw = true
		end
		if key("right", true) then
			cx = #lines[cy] + 1
			scroll_to_cursor()
			redraw = true
		end
		return
	end

	if key("escape", true) then
		mode = "browse"
		load_files()
		return
	end

	if hit("up") then
		move(0, -1)
	end
	if hit("down") then
		move(0, 1)
	end
	if hit("left") then
		move(-1, 0)
	end
	if hit("right") then
		move(1, 0)
	end
	if hit("pageup") then
		move(0, -BODY_ROWS)
	end
	if hit("pagedown") then
		move(0, BODY_ROWS)
	end
	if key("end", true) then
		cx = #lines[cy] + 1
		scroll_to_cursor()
		redraw = true
	end

	if hit("back") then
		backspace()
	end
	if hit("del") then
		del_fwd()
	end
	if key("enter", true) then
		newline()
	end
	if key("tab", true) then
		insert("\t")
	end

	local typed = cin()
	if #typed > 0 then
		if shift then
			typed = shift_str(typed)
		end
		insert(typed)
	end

	local m = mus()
	if m.m1 then
		local row = flr(m.y * ROWS) - BODY_TOP + 1
		if row >= 1 and row <= BODY_ROWS then
			local target = top + row - 1
			if target <= #lines then
				cy = target
				local want = flr(m.x * COLS) - GUTTER + left
				if want < 0 then
					want = 0
				end
				cx = col_from_disp(lines[cy], want)
				scroll_to_cursor()
				redraw = true
			end
		end
	end
end

-- Inverse of disp_col: which insertion point sits at this display column.
function col_from_disp(s, want)
	local d = 0
	for i = 1, #s do
		if d >= want then
			return i
		end
		if s:sub(i, i) == "\t" then
			d = d + TAB_W
		else
			d = d + 1
		end
	end
	return #s + 1
end

function render()
	clr()
	rect(0., 0., 1., 1., C_BG)
	if mode == "browse" then
		render_browse()
	else
		render_edit()
	end
	render_status()
end

function render_browse()
	rect(0, 0, COLS * CW, CH, C_BAR)
	-- Built two operands at a time: a longer chain still miscompiles in silt and
	-- surfaces as "userdata is not callable" on whichever line drew next.
	local head = "EDIT  " .. #files
	head = head .. " file(s) in the app below"
	text(head, 2, 1, C_TEXT)

	local from = 1
	if pick > BODY_ROWS then
		from = pick - BODY_ROWS + 1
	end
	for i = from, #files do
		local row = i - from + 1
		if row > BODY_ROWS then
			break
		end
		local y = flr((BODY_TOP + row - 1) * CH)
		local col = C_TEXT
		if i == pick then
			rect(0, y, COLS * CW, 8, C_BAR)
			col = C_PICK
		end
		-- Every computed argument is hoisted into a local first. A call in an
		-- argument list followed by a local argument miscompiles in silt: the
		-- callee register is clobbered and the call invokes its own first
		-- argument instead ("<first arg> is not callable"). test/silt-callarg
		-- has the minimal case.
		local gx = flr(CW)
		text(files[i], gx, y, col)
	end
end

function render_edit()
	rect(0, 0, COLS * CW, CH, C_BAR)
	local head = name
	if touched then
		head = "*" .. name
	end
	text(head, 2, 1, C_TEXT)

	for row = 1, BODY_ROWS do
		local n = top + row - 1
		if n > #lines then
			break
		end
		local y = flr((BODY_TOP + row - 1) * CH)
		local col = C_NUM
		if n == cy then
			col = C_NUM_ON
		end
		local num = pad(n)
		text(num, 0, y, col)
		local disp = expand(lines[n])
		paint_line(y, disp)
	end

	if (blink % 60) < 40 then
		local d = disp_col(lines[cy], cx) - left
		local row = cy - top + 1
		if d >= 0 and d < TEXT_COLS and row >= 1 and row <= BODY_ROWS then
			local x = flr((GUTTER + d) * CW)
			local y = flr((BODY_TOP + row - 1) * CH)
			rect(x, y, 1, 8, C_CURSOR)
		end
	end
end

-- Right-align a line number into the 3-wide gutter.
function pad(n)
	local s = "" .. n
	while #s < 3 do
		s = " " .. s
	end
	return s
end

function render_status()
	local y = flr(STATUS_ROW * CH)
	rect(0, y - 1, COLS * CW, CH, C_BAR)
	if mode == "browse" then
		text("enter opens   f5 re-lists", 2, y, C_HINT)
	else
		local pos = cy .. ":"
		pos = pos .. cx
		text(pos, 2, y, C_TEXT)
		if touched then
			local dx = flr(11 * CW)
			text("*", dx, y, C_DIRTY)
		end
		local hx = flr(15 * CW)
		text("ctrl+s save   esc files", hx, y, C_HINT)
	end
	if msg_left > 0 then
		local my = flr((STATUS_ROW - 1) * CH)
		text(msg, 2, my, C_PICK)
	end
end

-- One draw call per colour run. Glyphs are drawn with transparency, so painting a
-- second colour over the same cell would blend with the first instead of replacing
-- it — every character has to be drawn exactly once, in its final colour.
function paint_line(y, disp)
	local vislen = #disp - left
	if vislen > TEXT_COLS then
		vislen = TEXT_COLS
	end
	if vislen < 1 then
		return
	end
	local vis = disp:sub(left + 1, left + vislen)

	local col = {}
	for i = 1, vislen do
		col[i] = C_TEXT
	end

	local s_from = {}
	local s_to = {}
	scan_strings(disp, s_from, s_to)
	local com = comment_at(disp, s_from, s_to)
	mark_keywords(disp, col, vislen, com)
	for i = 1, #s_from do
		mark(col, vislen, s_from[i], s_to[i], C_STR)
	end
	if com ~= nil then
		mark(col, vislen, com, #disp, C_COM)
	end

	local i = 1
	while i <= vislen do
		local j = i
		while j < vislen do
			if col[j + 1] ~= col[i] then
				break
			end
			j = j + 1
		end
		local run = vis:sub(i, j)
		local rx = flr((GUTTER + i - 1) * CW)
		local rc = col[i]
		text(run, rx, y, rc)
		i = j + 1
	end
end

function mark(col, vislen, from, to, c)
	local a = from - left
	local b = to - left
	if b < 1 or a > vislen then
		return
	end
	if a < 1 then
		a = 1
	end
	if b > vislen then
		b = vislen
	end
	for i = a, b do
		col[i] = c
	end
end

function is_word(c)
	if c == "" then
		return false
	end
	if c:match("[%w_]") == nil then
		return false
	end
	return true
end

function mark_keywords(disp, col, vislen, stop)
	for k = 1, #KEYWORDS do
		local kw = KEYWORDS[k]
		local at = 1
		while true do
			local s = disp:find(kw, at, true)
			if s == nil then
				break
			end
			local e = s + #kw - 1
			local before = disp:sub(s - 1, s - 1)
			local after = disp:sub(e + 1, e + 1)
			if not is_word(before) and not is_word(after) then
				if stop == nil or e < stop then
					mark(col, vislen, s, e, C_KEY)
				end
			end
			at = s + 1
		end
	end
end

-- Quoted spans, so keywords inside a string aren't highlighted and a `--` inside
-- one isn't mistaken for a comment. Parallel arrays rather than a table of pairs.
function scan_strings(disp, s_from, s_to)
	local i = 1
	while i <= #disp do
		local c = disp:sub(i, i)
		if c == "\"" or c == "'" then
			local j = i + 1
			while j <= #disp do
				local d = disp:sub(j, j)
				if d == "\\" then
					j = j + 2
				elseif d == c then
					break
				else
					j = j + 1
				end
			end
			if j > #disp then
				j = #disp
			end
			s_from[#s_from + 1] = i
			s_to[#s_to + 1] = j
			i = j + 1
		else
			i = i + 1
		end
	end
end

function comment_at(disp, s_from, s_to)
	local at = 1
	while true do
		local s = disp:find("--", at, true)
		if s == nil then
			return nil
		end
		local inside = false
		for i = 1, #s_from do
			if s >= s_from[i] and s <= s_to[i] then
				inside = true
			end
		end
		if not inside then
			return s
		end
		at = s + 1
	end
end

function draw()
end

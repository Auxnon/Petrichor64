-- Minimal repro for a silt scoping fault.
--
-- Symptom: a call to a perfectly ordinary global (`key`) fails with
--   Value 'Value: userdata' is not callable
-- pointing at the call site. The value being called is *userdata*, and the only
-- userdata in scope is a global the app never named (`gui`/`sky`), so a lookup is
-- resolving to the wrong slot rather than the function being missing.
--
-- Each variant runs on its own frame, announced before it runs, so the log names the
-- culprit even though the error aborts the rest of loop(). Nothing needs pressing.

step = 0
KEYS = { "a", "b" }

function v1() -- bare global call
	if key("a", true) then
		cout("v1 pressed")
	end
end

function v2() -- inside a for loop
	for i = 1, 2 do
		if key("a", true) then
			cout("v2 pressed")
		end
	end
end

function v3() -- for-loop variable SHADOWS an existing local of the same name
	local i = 0
	for i = 1, 2 do
		if key("a", true) then
			cout("v3 pressed")
		end
	end
end

function v4() -- table index in the argument, no shadowing
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			cout("v4 pressed")
		end
	end
end

function v5() -- shadowing + table index (what the overlay demo actually did)
	local i = 0
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			cout("v5 pressed")
		end
	end
end

function v6() -- shadowing, but the shadowed local is READ before the loop
	local i = 7
	cout("v6 sees i=" .. tostring(i))
	for i = 1, 2 do
		if key("a", true) then
			cout("v6 pressed")
		end
	end
end


-- v7..v10: bisecting the overlay demo's real loop body, which DID fail. Each strips
-- one element to find what actually triggers it.
presses = 0
last_key = "-"
was_down = false

function v7() -- verbatim copy of the failing body
	local m = mus()
	local i = 0
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			last_key = KEYS[i]
			presses = presses + 1
		end
	end
	if m.m1 and not was_down then
		presses = presses + 1
		last_key = "click"
	end
	was_down = m.m1
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:text("v7", 0.09, 0.11, "DEF")
	line(m.x - 0.02, m.y, m.x + 0.02, m.y, "F5C")
end

function v8() -- no gui/line drawing
	local m = mus()
	local i = 0
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			presses = presses + 1
		end
	end
	if m.m1 and not was_down then
		presses = presses + 1
	end
	was_down = m.m1
end

function v9() -- drawing kept, but no shadowed local i
	local m = mus()
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			presses = presses + 1
		end
	end
	was_down = m.m1
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:text("v9", 0.09, 0.11, "DEF")
	line(m.x - 0.02, m.y, m.x + 0.02, m.y, "F5C")
end

function v10() -- shadowed local + drawing, but no mus()
	local i = 0
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			presses = presses + 1
		end
	end
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:text("v10", 0.09, 0.11, "DEF")
end


-- w1..w4: the demo's real loop body, trimmed by how many distinct globals it names.
-- The suspicion is that a function referencing enough distinct globals starts
-- resolving earlier ones to the wrong slot — `key` came back as userdata, and the only
-- userdata around is `gui`, which these bodies also name.
function w1() -- everything the demo's loop did
	local m = mus()
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			last_key = KEYS[i]
			presses = presses + 1
		end
	end
	if m.m1 and not was_down then
		presses = presses + 1
		last_key = "click"
	end
	was_down = m.m1
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:rect(0.06, 0.06, 0.88, 0.012, "5CF")
	gui:text("OVERLAY", 0.09, 0.11, "DEF")
	gui:text("input owner", 0.09, 0.16, "9CE")
	gui:text("presses: " .. tostring(presses) .. " last: " .. last_key, 0.09, 0.21, "FE9")
	gui:text("pointer: " .. tostring(flr(m.x * 100)), 0.09, 0.26, "9E9")
	gui:text("console", 0.09, 0.31, "888")
	line(m.x - 0.02, m.y, m.x + 0.02, m.y, "F5C")
	line(m.x, m.y - 0.02, m.x, m.y + 0.02, "F5C")
end

function w2() -- no line() calls
	local m = mus()
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			presses = presses + 1
		end
	end
	was_down = m.m1
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:rect(0.06, 0.06, 0.88, 0.012, "5CF")
	gui:text("presses: " .. tostring(presses), 0.09, 0.21, "FE9")
	gui:text("pointer: " .. tostring(flr(m.x * 100)), 0.09, 0.26, "9E9")
end

function w3() -- no tostring/flr either
	local m = mus()
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			presses = presses + 1
		end
	end
	was_down = m.m1
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:text("hello", 0.09, 0.21, "FE9")
end

function w4() -- key loop and clr only
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			presses = presses + 1
		end
	end
	clr()
end


-- x1..x4: threshold hunt. Same shape each time — one `key()` call followed by N
-- `gui:text` calls — varying only N. If an earlier global's resolution depends on how
-- much comes *after* it in the same function, this finds where it breaks.
function xN(n)
	if key("a", true) then
		presses = presses + 1
	end
	gui:text("1", 0.1, 0.10, "FFF")
	gui:text("2", 0.1, 0.14, "FFF")
	if n > 2 then
		gui:text("3", 0.1, 0.18, "FFF")
		gui:text("4", 0.1, 0.22, "FFF")
	end
	if n > 4 then
		gui:text("5", 0.1, 0.26, "FFF")
		gui:text("6", 0.1, 0.30, "FFF")
	end
	if n > 6 then
		gui:text("7", 0.1, 0.34, "FFF")
		gui:text("8", 0.1, 0.38, "FFF")
	end
end

-- Straight-line versions, in case the branching above changes the picture.
function x2()
	if key("a", true) then presses = presses + 1 end
	gui:text("1", 0.1, 0.10, "FFF")
	gui:text("2", 0.1, 0.14, "FFF")
end

function x4()
	if key("a", true) then presses = presses + 1 end
	gui:text("1", 0.1, 0.10, "FFF")
	gui:text("2", 0.1, 0.14, "FFF")
	gui:text("3", 0.1, 0.18, "FFF")
	gui:text("4", 0.1, 0.22, "FFF")
end

function x6()
	if key("a", true) then presses = presses + 1 end
	gui:text("1", 0.1, 0.10, "FFF")
	gui:text("2", 0.1, 0.14, "FFF")
	gui:text("3", 0.1, 0.18, "FFF")
	gui:text("4", 0.1, 0.22, "FFF")
	gui:text("5", 0.1, 0.26, "FFF")
	gui:text("6", 0.1, 0.30, "FFF")
end

function x8()
	if key("a", true) then presses = presses + 1 end
	gui:text("1", 0.1, 0.10, "FFF")
	gui:text("2", 0.1, 0.14, "FFF")
	gui:text("3", 0.1, 0.18, "FFF")
	gui:text("4", 0.1, 0.22, "FFF")
	gui:text("5", 0.1, 0.26, "FFF")
	gui:text("6", 0.1, 0.30, "FFF")
	gui:text("7", 0.1, 0.34, "FFF")
	gui:text("8", 0.1, 0.38, "FFF")
end


-- y1..y3: is the trigger the *global alias* forms (`line`, `fill`, `rect`) — which the
-- engine exposes as bare globals that stand in for `gui:line` etc — appearing in the
-- same function as another global call?
function y1() -- key + the line() alias
	if key("a", true) then
		presses = presses + 1
	end
	line(0.1, 0.1, 0.9, 0.1, "F5C")
end

function y2() -- key + gui:line (method form, not the alias)
	if key("a", true) then
		presses = presses + 1
	end
	gui:line(0.1, 0.1, 0.9, 0.1, "F5C")
end

function y3() -- key + clr + line alias, closest to w1's mix
	if key("a", true) then
		presses = presses + 1
	end
	clr()
	line(0.1, 0.1, 0.9, 0.1, "F5C")
	line(0.1, 0.2, 0.9, 0.2, "F5C")
end


-- z1..z4: w1 with exactly one element removed, to isolate the trigger.
function z1() -- w1 minus `last_key = KEYS[i]` in the key loop
	local m = mus()
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			presses = presses + 1
		end
	end
	if m.m1 and not was_down then
		presses = presses + 1
		last_key = "click"
	end
	was_down = m.m1
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:rect(0.06, 0.06, 0.88, 0.012, "5CF")
	gui:text("OVERLAY", 0.09, 0.11, "DEF")
	gui:text("input owner", 0.09, 0.16, "9CE")
	gui:text("presses: " .. tostring(presses) .. " last: " .. last_key, 0.09, 0.21, "FE9")
	gui:text("pointer: " .. tostring(flr(m.x * 100)), 0.09, 0.26, "9E9")
	gui:text("console", 0.09, 0.31, "888")
	line(m.x - 0.02, m.y, m.x + 0.02, m.y, "F5C")
	line(m.x, m.y - 0.02, m.x, m.y + 0.02, "F5C")
end

function z2() -- w1 minus the m.m1 / was_down block
	local m = mus()
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			last_key = KEYS[i]
			presses = presses + 1
		end
	end
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:rect(0.06, 0.06, 0.88, 0.012, "5CF")
	gui:text("OVERLAY", 0.09, 0.11, "DEF")
	gui:text("input owner", 0.09, 0.16, "9CE")
	gui:text("presses: " .. tostring(presses) .. " last: " .. last_key, 0.09, 0.21, "FE9")
	gui:text("pointer: " .. tostring(flr(m.x * 100)), 0.09, 0.26, "9E9")
	gui:text("console", 0.09, 0.31, "888")
	line(m.x - 0.02, m.y, m.x + 0.02, m.y, "F5C")
	line(m.x, m.y - 0.02, m.x, m.y + 0.02, "F5C")
end

function z3() -- w1 minus the concatenated text lines
	local m = mus()
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			last_key = KEYS[i]
			presses = presses + 1
		end
	end
	if m.m1 and not was_down then
		presses = presses + 1
		last_key = "click"
	end
	was_down = m.m1
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:rect(0.06, 0.06, 0.88, 0.012, "5CF")
	gui:text("OVERLAY", 0.09, 0.11, "DEF")
	gui:text("input owner", 0.09, 0.16, "9CE")
	gui:text("console", 0.09, 0.31, "888")
	line(m.x - 0.02, m.y, m.x + 0.02, m.y, "F5C")
	line(m.x, m.y - 0.02, m.x, m.y + 0.02, "F5C")
end

function z4() -- w1 minus the two line() calls
	local m = mus()
	for i = 1, #KEYS do
		if key(KEYS[i], true) then
			last_key = KEYS[i]
			presses = presses + 1
		end
	end
	if m.m1 and not was_down then
		presses = presses + 1
		last_key = "click"
	end
	was_down = m.m1
	clr()
	gui:rrect(0.06, 0.06, 0.88, 0.30, 0.02, "123")
	gui:rect(0.06, 0.06, 0.88, 0.012, "5CF")
	gui:text("OVERLAY", 0.09, 0.11, "DEF")
	gui:text("input owner", 0.09, 0.16, "9CE")
	gui:text("presses: " .. tostring(presses) .. " last: " .. last_key, 0.09, 0.21, "FE9")
	gui:text("pointer: " .. tostring(flr(m.x * 100)), 0.09, 0.26, "9E9")
	gui:text("console", 0.09, 0.31, "888")
end


-- c1..c4: concat chain length is the suspect. Identical functions apart from how many
-- `..` operators appear in one expression. Note the error is reported at the `key`
-- call, which comes *before* the concat — so if this is it, a long chain is corrupting
-- the enclosing function's compilation, not just its own statement.
function c1()
	if key("a", true) then presses = presses + 1 end
	gui:text("a" .. "b", 0.1, 0.10, "FFF")
end

function c2()
	if key("a", true) then presses = presses + 1 end
	gui:text("a" .. "b" .. "c", 0.1, 0.10, "FFF")
end

function c3()
	if key("a", true) then presses = presses + 1 end
	gui:text("a" .. "b" .. "c" .. "d", 0.1, 0.10, "FFF")
end

function c4()
	if key("a", true) then presses = presses + 1 end
	gui:text("a" .. tostring(presses) .. "c" .. last_key, 0.1, 0.10, "FFF")
end

function main()
	cout("silt scope repro: watch for 'not callable'")
end

function loop()
	-- Advance first: an error inside a variant aborts the rest of this function, so
	-- incrementing afterwards would retry the same variant forever.
	step = step + 1
	if step == 2 then
		cout("--- v1 bare call")
		v1()
	elseif step == 4 then
		cout("--- v2 for loop")
		v2()
	elseif step == 6 then
		cout("--- v3 shadowed loop var")
		v3()
	elseif step == 8 then
		cout("--- v4 table index")
		v4()
	elseif step == 10 then
		cout("--- v5 shadow + table index")
		v5()
	elseif step == 12 then
		cout("--- v6 shadow, read before loop")
		v6()
	elseif step == 14 then
		cout("--- v7 verbatim failing body")
		v7()
	elseif step == 16 then
		cout("--- v8 no drawing")
		v8()
	elseif step == 18 then
		cout("--- v9 no shadowed local")
		v9()
	elseif step == 20 then
		cout("--- v10 no mus()")
		v10()
	elseif step == 22 then
		cout("--- w1 full demo body")
		w1()
	elseif step == 24 then
		cout("--- w2 no line()")
		w2()
	elseif step == 26 then
		cout("--- w3 no tostring/flr")
		w3()
	elseif step == 28 then
		cout("--- w4 keys + clr only")
		w4()
	elseif step == 32 then
		cout("--- x2")
		x2()
	elseif step == 34 then
		cout("--- x4")
		x4()
	elseif step == 36 then
		cout("--- x6")
		x6()
	elseif step == 38 then
		cout("--- x8")
		x8()
	elseif step == 42 then
		cout("--- y1 key + line alias")
		y1()
	elseif step == 44 then
		cout("--- y2 key + gui:line")
		y2()
	elseif step == 46 then
		cout("--- y3 key + clr + 2 line")
		y3()
	elseif step == 50 then
		cout("--- z1 no last_key in loop")
		z1()
	elseif step == 52 then
		cout("--- z2 no m1 block")
		z2()
	elseif step == 54 then
		cout("--- z3 no concat text")
		z3()
	elseif step == 56 then
		cout("--- z4 no line calls")
		z4()
	elseif step == 60 then
		cout("--- c1 one concat")
		c1()
	elseif step == 62 then
		cout("--- c2 two concats")
		c2()
	elseif step == 64 then
		cout("--- c3 three concats")
		c3()
	elseif step == 66 then
		cout("--- c4 three concats w/ calls+global")
		c4()
	elseif step == 68 then
		cout("--- done")
	end
end

function draw() end

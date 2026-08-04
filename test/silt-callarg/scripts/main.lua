-- silt repro harness: miscompiles found while writing the editor overlay (apps/edit)
-- and the modeller (apps/model). Run it and read the console — every check prints ok or FAIL, and a
-- miscompile shows up as an "is not callable" error rather than a FAIL line.
--
--     Petrichor64 test/silt-callarg
--
-- 1. CALL ARGUMENTS — a call in an argument list, followed by a local-variable
--    argument, clobbers the callee register: the call invokes its own FIRST
--    ARGUMENT instead of the function. `text("lit", flr(8), y, col)` reports
--    Value 'Value: "lit"' is not callable. Hoisting the inner call into a local
--    first is a clean workaround, which is what apps/edit does throughout.
--
-- 2. table.insert(t, pos, v) OVERWRITES on a table grown with t[#t+1]: the element
--    at `pos` is lost and # never grows. A table written as a {...} constructor
--    behaves correctly, so this only bites tables built up in a loop — which is
--    what any line buffer, tokenizer, or file list is.
--
-- 3. table.remove(t, pos) leaves a NIL HOLE inside the array while decrementing #,
--    so a later t[i] returns nil for an i < #t.
--
-- Also seen but NOT exercised here, because it aborts the Lua thread outright:
-- ~60 top-level `NAME = value` globals in one file panics in
-- silt/src/lua.rs:1601, OpCode::DEFINE_GLOBAL — "attempt to subtract with
-- overflow" on `self.stack_count -= 1` with the stack already empty.
-- To reproduce: append 60 blocks of `G<i> = "c"` + a small function to any app.
--
-- 5. A table CONSTRUCTOR holding a local, assigned straight into a
--    length-computed index, corrupts the compiled chunk: "Invalid chunk due
--    compilation corruption". Constants in the same position are fine, and
--    hoisting the constructor into a local first (or using table.insert) works.
--    Found building apps/model, where `q[#q + 1] = { a - LINE, -e, 0 }` in a mesh
--    builder killed the whole file. STILL OPEN.
--
-- Full write-up, with the silt source locations for each: SILT-BUGS.md

step = 0
T = { "a.lua", "b.lua" }
GRID = 8
CW = 8
C = "DDE"
fails = 0

function ok(label, got, want)
	if got == want then
		cout("ok", label, got)
	else
		fails = fails + 1
		cout("FAIL", label, "got", got, "want", want)
	end
end

-- 1. call-argument register clobber -----------------------------------------

function c_ok_literals() -- index + nested call + literals: fine
	text(T[1], flr(CW), 10, C)
end

function c_ok_locals() -- index + locals, no nested call: fine
	local y = 10
	local col = C
	text(T[1], CW, y, col)
end

function c_bad_call_then_local() -- nested call THEN a local argument: miscompiles
	local y = 10
	local col = C
	text("lit", flr(CW), y, col)
end

function c_bad_index_call_local() -- the shape apps/edit kept hitting
	local y = 10
	local col = C
	text(T[1], flr(CW), y, col)
end

function c_fix_hoisted() -- same call with the inner call hoisted: fine
	local y = 10
	local col = C
	local x = flr(CW)
	text(T[1], x, y, col)
end

-- 2 and 3. table.insert / table.remove ---------------------------------------

function build(n)
	local t = {}
	for i = 1, n do
		t[#t + 1] = "line" .. i
	end
	return t
end

function holes(t)
	local bad = 0
	for i = 1, #t do
		if t[i] == nil then
			bad = bad + 1
		end
	end
	return bad
end

function main()
	cout("silt repro harness — watch for 'is not callable'")
end

function loop()
	step = step + 1

	if step == 20 then
		cout("--- c1 index + call + literals (expected fine)")
		c_ok_literals()
	elseif step == 40 then
		cout("--- c2 index + locals (expected fine)")
		c_ok_locals()
	elseif step == 60 then
		cout("--- c3 call then local arg (EXPECTED MISCOMPILE)")
		c_bad_call_then_local()
	elseif step == 80 then
		cout("--- c4 index + call + locals (EXPECTED MISCOMPILE)")
		c_bad_index_call_local()
	elseif step == 100 then
		cout("--- c5 inner call hoisted (expected fine)")
		c_fix_hoisted()
	elseif step == 120 then
		cout("--- t1 table.insert in the middle of a loop-built table")
		local t = build(19)
		table.insert(t, 2, "INS")
		ok("len grows to 20", #t, 20)
		ok("line2 survives at index 3", t[3], "line2")
	elseif step == 140 then
		cout("--- t2 table.remove from a loop-built table")
		local t = build(19)
		table.remove(t, 2)
		ok("len drops to 18", #t, 18)
		ok("no nil holes", holes(t), 0)
		ok("line3 shifts down to index 2", t[2], "line3")
	elseif step == 155 then
		cout("--- k1 constructor with a local into t[#t+1] (EXPECTED CORRUPTION)")
		local q = {}
		local e = GRID
		q[#q + 1] = { -e, 0, 0 }
		ok("ctor with a local", #q, 1)
	elseif step == 158 then
		cout("--- k2 same, constants only (expected fine)")
		local q = {}
		q[#q + 1] = { 1, 2, 3 }
		ok("ctor with constants", #q, 1)
	elseif step == 159 then
		cout("--- k3 same, via table.insert (the workaround)")
		local q = {}
		local e = GRID
		table.insert(q, { -e, 0, 0 })
		ok("table.insert with a local", #q, 1)
	elseif step == 160 then
		cout("--- t3 the same two through a rebuild (the workaround)")
		local t = build(19)
		local out = {}
		for i = 1, #t do
			if i == 2 then
				out[#out + 1] = "INS"
			end
			out[#out + 1] = t[i]
		end
		ok("rebuild len", #out, 20)
		ok("rebuild keeps line2", out[3], "line2")
	elseif step == 180 then
		cout("--- done, failures:", fails)
	end
end

function draw()
end

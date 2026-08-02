-- What a source editor can rely on. One check per step so a crash localizes, and
-- results go through cout's variadic form rather than concat, so a concat bug can't
-- be mistaken for a string-library gap. Run as an overlay to also cover `key` and
-- `app.*`:  Petrichor64 test/target --overlay test/strings
step = 0
s = "local x = 42 -- hi"

function ok(label, got, want)
	if got == want then
		cout("ok", label, got)
	else
		cout("FAIL", label, "got", got, "want", want)
	end
end

function main()
	cout("string probe")
end

function loop()
	step = step + 1

	if step == 10 then
		ok("len", #s, 18)
		ok("sub", s:sub(1, 5), "local")
		ok("sub neg", s:sub(-2), "hi")
	elseif step == 20 then
		ok("find plain", s:find("x =", 1, true), 7)
		ok("find pattern", s:find("%d+"), 11)
		ok("find miss", s:find("zzz", 1, true), nil)
	elseif step == 30 then
		ok("match word", s:match("%a+"), "local")
		ok("match capture", s:match("(%d+)"), "42")
		ok("match trim", ("  pad  "):match("^%s*(.-)%s*$"), "pad")
	elseif step == 40 then
		ok("gsub", ("a.b.c"):gsub("%.", "/"), "a/b/c")
		ok("gsub plain-ish", ("xx"):gsub("x", "y"), "yy")
	elseif step == 50 then
		ok("format", string.format("%d:%s", 7, "hi"), "7:hi")
		ok("rep", ("ab"):rep(3), "ababab")
		ok("upper", ("hi"):upper(), "HI")
	elseif step == 60 then
		ok("byte", ("A"):byte(), 65)
		ok("char", string.char(66), "B")
		ok("reverse", ("abc"):reverse(), "cba")
	elseif step == 70 then
		-- gmatch is the one an editor would reach for to split lines; report it
		-- rather than assume, since a manual find loop is the fallback.
		if string.gmatch == nil then
			cout("gmatch MISSING (use a find loop)")
		else
			cout("gmatch present")
		end
	elseif step == 80 then
		-- Splitting a buffer into lines without gmatch: the fallback an editor
		-- would actually ship.
		local text = "one\ntwo\nthree"
		local lines = {}
		local at = 1
		while true do
			local nl = text:find("\n", at, true)
			if nl == nil then
				lines[#lines + 1] = text:sub(at)
				break
			end
			lines[#lines + 1] = text:sub(at, nl - 1)
			at = nl + 1
		end
		ok("split count", #lines, 3)
		ok("split first", lines[1], "one")
		ok("split last", lines[3], "three")
	elseif step == 90 then
		-- Table shapes the old editor used, including the constructor styles that
		-- used to fail outright.
		local t = {
			a = 1,
			b = 2,
		}
		ok("multiline table", t.b, 2)
		local arr = { "x", "y" }
		table.insert(arr, "z")
		ok("insert", #arr, 3)
		table.remove(arr, 1)
		ok("remove", arr[1], "y")
	elseif step == 100 then
		-- The reported blocker: `key` came through as userdata in an overlay VM.
		cout("key type", type(key))
		if key("a", true) then
			cout("key said yes")
		else
			cout("key callable, returned false")
		end
	elseif step == 110 then
		cout("cin type", type(cin))
		local typed = cin()
		cout("cin returned", type(typed), #typed)
	elseif step == 120 then
		cout("app.read type", type(app.read))
		local src = app.read("scripts/main.lua")
		if src == nil then
			cout("app.read FAILED")
		else
			ok("app.read is a string", type(src), "string")
		end
	elseif step == 130 then
		-- Exactly the table ops a line buffer needs: insert/remove in the middle,
		-- and whether `#` tracks both.
		local t = { "a", "c" }
		table.insert(t, 2, "b")
		ok("insert at pos", t[2], "b")
		ok("len after insert", #t, 3)
		table.remove(t, 1)
		ok("len after remove", #t, 2)
		ok("shifted down", t[1], "b")
	elseif step == 140 then
		if table.concat == nil then
			cout("table.concat MISSING (join by hand)")
		else
			ok("concat", table.concat({ "x", "y" }, "-"), "x-y")
		end
		local u = { "a", "b" }
		u[#u] = nil
		ok("nil shrinks len", #u, 1)
	elseif step == 150 then
		cout("probe done")
	end
end

function draw()
end

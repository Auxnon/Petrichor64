-- Overlay-side probe for the `app.*` natives: list, read, write, escape, reload.
-- One step per frame so each result lands on its own console line, and so a crash
-- points at a single call. Run it as an overlay over a throwaway copy of an app:
--   Petrichor64 <copy of test/target> --overlay test/appapi
step = 0

function main()
	cout("app api probe")
	if app == nil then
		cout("FAIL: overlay has no app table")
	end
end

function loop()
	step = step + 1
	if step == 20 then
		local files = app.list()
		cout("list count")
		cout(#files)
		for i = 1, #files do
			cout(files[i])
		end
	elseif step == 40 then
		local src = app.read("scripts/main.lua")
		if src == nil then
			cout("read FAILED")
		else
			cout("read bytes")
			cout(#src)
		end
	elseif step == 60 then
		cout("write")
		cout(app.write("probe.txt", "hello from overlay"))
	elseif step == 80 then
		cout("roundtrip")
		cout(app.read("probe.txt"))
	elseif step == 100 then
		-- Must be refused: climbing out of the app folder is the whole point of
		-- scrubbing the path.
		cout("escape read")
		cout(app.read("../../Cargo.toml"))
	elseif step == 120 then
		cout("escape write")
		cout(app.write("../../ESCAPED.txt", "nope"))
	elseif step == 140 then
		-- The app under us should print "target loaded" a second time, and this
		-- overlay should survive it.
		cout("reload")
		cout(app.reload())
	elseif step == 200 then
		cout("overlay still alive after reload")
	end
end

function draw()
end

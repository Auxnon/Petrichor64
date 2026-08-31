-- Loop-rate probe: prints a tick every 30 loop calls. Count ticks over a known
-- wall-clock window to get the Lua loop rate, which is not the same thing as the
-- render fps the engine reports.
n = 0
ticks = 0

function main()
	cout("tick probe running")
end

function loop()
	n = n + 1
	if n >= 30 then
		n = 0
		ticks = ticks + 1
		cout("tick")
	end
end

function draw() end

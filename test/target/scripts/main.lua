-- The app *under* an overlay. Two jobs: prove a game cannot see the privileged
-- `app` table, and be visibly reloadable (count the "target loaded" lines).
loads = 0

function main()
	cout("target loaded")
	if app == nil then
		cout("target: no app table (correct)")
	else
		cout("target: HAS app table (WRONG)")
	end
end

function loop()
end

function draw()
end

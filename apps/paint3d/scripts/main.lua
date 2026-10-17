-- PAINT3D: the Ctrl+P overlay. Point at anything in the running app, click it,
-- and paint the texture it shows, right on the mesh.
--
-- The app is paused underneath. Clicking a mesh, sprite or tile focuses it:
-- app.pick finds the texture and the exact texel under the pointer, and a tool
-- panel floats in 3D beside the spot you clicked. Drag on the focused thing to
-- paint; every stroke lands live on the mesh (and on every other mesh sharing
-- that texture). Click a different mesh to move focus. ZOOM (or C) blows the
-- texture up into a flat pixel canvas; the tools, colour and brush carry over,
-- and the scene stays visible beside it so you can watch the mesh update.
--
-- Keys: B pen  F fill  I pick colour  1-3 brush size  Z undo  C canvas
--       Esc steps back (canvas -> focus -> close)  Ctrl+P closes
--
-- A dithered frame marks the overlay as open. Once something is painted, a
-- SAVE button in the lower right asks to write the edits back into the app's
-- image files (save.lua).
--
-- Written in the restricted style AGENTS.md asks for (silt): calls hoisted out
-- of argument lists, no anonymous functions, no table deletion.

-- Shared state lives in a few tables (see pal.lua/panel.lua/paint.lua).
P = { tool = "pen", color = "ffffffff", size = 1, mode = "scene" }
F = { on = false, key = "", tex = "", tw = 1, th = 1, x = 0, y = 0, z = 0, img = nil }
U = { last_m1 = false, painting = false, lastu = -1, lastv = -1, msg = "", msg_t = 0, panel_side = 1 }
H = { n = 0, img = {}, tex = {} }

HIDE_Z = -10000

function main()
	-- Panels draw over the scene, never inside it.
	attr({ front = true })
	app.pause(true)
	textures_init()
	U.panel = make("px3_panel", 0, 0, HIDE_Z, 1)
	U.cursor = make("px3_cur", 0, 0, HIDE_Z, 0.1)
	U.pin = make("px3_pin", 0, 0, HIDE_Z, 0.1)
	-- true colours: the app's lights and fog shouldn't shade the tools
	local a = U.panel
	a.unlit = true
	local b = U.cursor
	b.unlit = true
	local c = U.pin
	c.unlit = true
	pal_reset()
	panel_draw()
	say("CLICK SOMETHING TO PAINT IT")
end

function loop(dt)
	local dialog = dialog_keys()
	if not dialog then
		keys_update()
	end
	local m = mus()
	local click = false
	if m.m1 and not U.last_m1 then
		click = true
	end
	U.last_m1 = m.m1
	if not m.m1 then
		U.painting = false
		U.lastu = -1
	end
	-- the frame's button and dialog sit over everything, so they see clicks first
	if click then
		local gx = flr(m.x * gui.w)
		local gy = flr(m.y * gui.h)
		local took = chrome_click(gx, gy)
		if took then
			click = false
		end
	end
	-- while the dialog is up nothing underneath reacts
	if not D.open and not dialog then
		if P.mode == "canvas" then
			canvas_update(m, click)
		else
			scene_update(m, click)
		end
	end
	if U.msg_t > 0 then
		U.msg_t = U.msg_t - 1
	end
	status_draw()
	chrome_draw()
	-- scripted playtests drop a zz_test.lua defining this (see AGENTS.md)
	if TEST_HOOK ~= nil then
		TEST_HOOK(dt)
	end
end

function draw() end

function say(s)
	U.msg = s
	U.msg_t = 180
end

function keys_update()
	if key("escape", true) then
		escape()
	end
	if key("b", true) then
		set_tool("pen")
	end
	if key("f", true) then
		set_tool("fill")
	end
	if key("i", true) then
		set_tool("pick")
	end
	if key("1", true) then
		set_size(1)
	end
	if key("2", true) then
		set_size(2)
	end
	if key("3", true) then
		set_size(3)
	end
	if key("z", true) then
		undo()
	end
	if key("c", true) then
		toggle_canvas()
	end
end

function escape()
	if P.mode == "canvas" then
		toggle_canvas()
	elseif F.on then
		unfocus()
	else
		app.close()
	end
end

function set_tool(t)
	P.tool = t
	panel_draw()
end

function set_size(n)
	P.size = n
	panel_draw()
end

function set_color(c)
	P.color = c
	panel_draw()
end

function toggle_canvas()
	if P.mode == "canvas" then
		P.mode = "scene"
		panel_place()
		return
	end
	if not F.on then
		say("FOCUS SOMETHING FIRST")
		return
	end
	P.mode = "canvas"
	hide(U.panel)
	hide(U.cursor)
end

function hide(e)
	e.z = HIDE_Z
end

-- A stable id for "the thing that was clicked": one entity, or the tile world.
function target_key(h)
	if h.kind == "tile" then
		return "tile"
	end
	local k = tostring(h.id)
	k = "ent" .. k
	return k
end

-- Status line along the top of the overlay's own gui layer. Everything else in
-- scene mode is left clear so the app shows through.
function status_draw()
	if P.mode == "canvas" then
		return
	end
	clr()
	local w = gui.w
	local bw = w - 2 * INSET
	gui:rect(INSET, INSET, bw, 11, "151722ff")
	local s = "PAINT"
	if F.on then
		local t = short(F.tex, 20)
		s = s .. "  "
		s = s .. t
	end
	if U.msg_t > 0 then
		s = s .. "  "
		s = s .. U.msg
	end
	local tx = INSET + 3
	local ty = INSET + 2
	gui:text(s, tx, ty, "e8e8f0")
	local hx = w - 84 - INSET
	gui:text("ESC/CTRL+P", hx, ty, "8a8fa8")
end

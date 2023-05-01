-- Codex 3.0.0 "Artichoke"
-- example = make('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
current_message = ""
last_message = ""
panels = {}
counter = 1
tag = "" .. irnd(9) .. irnd(9) .. irnd(9) .. irnd(9)

who = {}
function make_panel(id)
    local im = nimg(128, 64)
    color = { rnd(.5, 1), rnd(.5, 1), rnd(.5, 1) }
    im:fill(color)
    im:text("...", 0, 0, "000")
    -- local id = counter
    tex("panel" .. id, im)
    local ent = make("p", rnd(-2, 2), 6, rnd(-2, 2))
    ent.tex = "panel" .. id
    ent.rz = tau / 8

    panels[counter] = { im = im, color = color, id = id, ent = ent }
    who[id] = counter
    counter = counter + 1
end

function main()
    mod("p", { q = { { 0, 0, 0 }, { 1, 0, 0 }, { 1, 0, .5 }, { 0, 0, .5 } }, t = { "panel" } })
    make_panel(tag)
    servy = conn("192.168.1.42:3000")
    cout 'client running'
end

function loop()
    if key("return", true) then
        -- local m = "hi"
        -- local r = rnd()
        -- if r < .25 then
        --     m = "hello"
        -- elseif r < .5 then
        --     m = "hi"
        -- elseif r < .75 then
        --     m = "howdy"
        -- else
        --     m = "hey"
        -- end
        servy:send(tag .. ":" .. current_message)
        cout("sent " .. current_message)
        last_message = current_message
        current_message = ""
        -- for i = 1, #panels do
        --     local p = panels[i]
        --     p.ent.rz = p.ent.rz + tau / 16
        --     cout("rotating " .. p.ent.rz)
        --     -- p.ent:destroy()
        -- end
    else
        current_message = current_message .. cin()
    end

    local r = servy:recv()
    if r then
        cout("got", r)
        local id, msg = r:match("^(%d+):(.*)$")
        if who[id] == nil then
            make_panel(id)
        end
        local p = panels[who[id]]
        p.im:fill(p.color)
        p.im:text(msg, 0, 0, "000")
        tex("panel" .. id, p.im)

        -- servy:send(r)
    end
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
end

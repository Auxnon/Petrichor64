attr { fog = 200. }
sky()
fill("333")
-- pixel(0, 0, "F00")
-- pixel(320, 280, "0F0")
floors = {}
rain = {}
drops = {}
drop_it = 1
state = -1

function make_model()
    local mx = 0.5
    smodel("console", { q = {
        { mx, -mx, .5 }, { mx, mx, .5 }, { -mx, mx, .5 }, { -mx, -mx, .5 },
        { -mx, -mx, .5 }, { -mx, mx, .5 }, { -mx, mx, 0 }, { -mx, -mx, 0 },
        { -mx, mx, .5 }, { mx, mx, .5 }, { mx, mx, 0 }, { -mx, mx, 0 },
    }, t = { "logo13", "logo17", "logo12" } })
    smodel("ground", { q = {
        { -mx, -mx, .5 }, { mx, -mx, .5 }, { mx, mx, .5 }, { -mx, mx, .5 },
        { -mx, -mx, .5 }, { -mx, mx, .5 }, { -mx, mx, 0 }, { -mx, -mx, 0 },
        { -mx, mx, .5 }, { mx, mx, .5 }, { mx, mx, 0 }, { -mx, mx, 0 },
    }, t = { "logo11", "logo10", "logo10" } })
    smodel("drops", { q = {

        { -mx, -mx, 1.5 }, { mx, mx, 1.5 }, { mx, mx, .5 }, { -mx, -mx, .5 },

    }, t = { "logo18" } })
    smodel("clouds", { q = {
        { -mx, -mx, 1.5 }, { mx, -mx, 1.5 }, { mx, mx, 1.5 }, { -mx, mx, 1.5 },
        { -mx, -mx, 1.625 }, { mx, -mx, 1.625 }, { mx, mx, 1.625 }, { -mx, mx, 1.625 },
        { -mx, -mx, 1.75 }, { mx, -mx, 1.75 }, { mx, mx, 1.75 }, { -mx, mx, 1.75 },
        { -mx, -mx, 1.875 }, { mx, -mx, 1.875 }, { mx, mx, 1.875 }, { -mx, mx, 1.875 },
        { -mx, -mx, 2 }, { mx, -mx, 2 }, { mx, mx, 2 }, { -mx, mx, 2 },
    }, t = { "logo16" } })
    -- smodel("logo-ground", { q = {
    --     { -mx, -mx, .5 }, { mx, -mx, .5 }, { mx, mx, .5 }, { -mx, mx, .5 },
    --     { -mx, -mx, .5 }, { -mx, mx, .5 }, { -mx, mx, 0 }, { -mx, -mx, 0 },
    --     { -mx, mx, .5 }, { mx, mx, .5 }, { mx, mx, 0 }, { -mx, mx, 0 },

    --     { -mx, -mx, 1.5 }, { mx, mx, 1.5 }, { mx, mx, .5 }, { -mx, -mx, .5 },

    --     { -mx, -mx, 1.5 }, { mx, -mx, 1.5 }, { mx, mx, 1.5 }, { -mx, mx, 1.5 },
    --     { -mx, -mx, 1.625 }, { mx, -mx, 1.625 }, { mx, mx, 1.625 }, { -mx, mx, 1.625 },
    --     { -mx, -mx, 1.75 }, { mx, -mx, 1.75 }, { mx, mx, 1.75 }, { -mx, mx, 1.75 },
    --     { -mx, -mx, 1.875 }, { mx, -mx, 1.875 }, { mx, mx, 1.875 }, { -mx, mx, 1.875 },
    --     { -mx, -mx, 2 }, { mx, -mx, 2 }, { mx, mx, 2 }, { -mx, mx, 2 },

    -- }, t = { "logo11", "logo10", "logo10", "logo18", "logo16", "logo16", "logo16", "logo16", "logo16" } })
end

function floor()
    for i = -6, 6 do
        for j = -6, 5 do
            local e = spawn("cube", i, j, 0)
            e:stex("logo" .. irnd(20, 24))
            floors[#floors + 1] = e
        end
    end
end

function make_rain()
    for i = 1, 100 do
        local e = spawn("logo14", rnd(-6, 6), rnd(-6, 6), rnd(7) - 1)
        rain[#rain + 1] = e
    end
    for i = 1, 60 do
        local e = spawn("plop", rnd(-6, 6), rnd(-6, 6), 8)
        drops[#drops + 1] = e
    end
end

function floor_loop()
    for i = 1, #floors do
        local e = floors[i]
        e.y = e.y - 0.075
        -- e.z = e.z + 0.002
        if e.y < -6 then
            e.y = 6
            -- e.z = 0
        end
    end

end

function rain_loop()

    for i = 1, #rain do
        local e = rain[i]
        e.z = e.z - 0.1
        e.y = e.y - 0.075
        if e.z < -1 then
            local d = drops[drop_it]
            d.x = e.x
            d.y = e.y
            d.z = 1.5
            d:anim("plop")
            drop_it = drop_it + 1
            if drop_it > #drops then
                drop_it = 1
            end
            e.z = 6
            e.y = rnd(-6, 6)
        end
    end
    for i = 1, #drops do
        local e = drops[i]
        e.y = e.y - 0.075
        -- if e.y < -1 then
        --     e.z = 6
        -- end
    end
end

function main()
    local t1 = "Drag and drop game file"
    local t2 = "` opens console, type 'help'"
    text(t1, "=50% -" .. flr(t1:len() * 10 / 2), 8)
    text(t2, "=50% -" .. flr(t2:len() * 10 / 2), "=100% - 12")
    anim("plop", { "logo6", "logo7", "logo8", "logo9" })
    make_model()

    console = spawn("console", 0, 0, 1.125)
    console.rz = 6 * tau / 16
    console.rx = tau / 48

    cam { pos = { 0, -8, 3 }, rot = { pi / 2, -0.1 } }
end

function drop()
    clr()
    console:kill()
    logo1 = spawn("ground", 0, 0, 1.125)
    logoA = spawn("drops", -0.25, 0, 8.125)
    logoB = spawn("drops", 0, 0, 8.125)
    logoC = spawn("drops", 0.25, 0, 8.125)
    logo3 = spawn("clouds", 0, 0, 4.125)

    logo1.rz = 6 * tau / 16
    logo1.rx = tau / 48
    logo3.rz = 6 * tau / 16
    logo3.rx = tau / 48
    state = 0
end

function make_title()
    fill("FFF")
    floor()
    make_rain()
    local h = 4
    local pe = spawn("logo0", -2.25, 0, h)
    local tr = spawn("logo1", -1.25, 0, h)
    local ic = spawn("logo2", -.25, 0, h)
    local ho = spawn("logo3", .75, 0, h)
    local r = spawn("logo4", 1.75, 0, h)
    local l64 = spawn("logo5", 2.25, 0, h)
    gui()
    local t = "Interpreted Game System"
    text(t, "=50% - " .. flr(10 * t:len() / 2), "=100% -20")
end

function loop()
    floor_loop()
    rain_loop()
    if state == 0 then

        logo3.z = logo3.z - 0.05
        if logo3.z <= 1.125 then
            state = 1
            logo3.z = 1.125
            logoA.z = 1.75
        end
    elseif state == 1 then
        logoA.z = logoA.z - 0.025
        if logoA.z <= 1.25 then
            state = 3
            logoA.z = 1.25
            logoB.z = 1.75
        end
    elseif state == 3 then
        logoB.z = logoB.z - 0.025
        if logoB.z <= 1.125 then
            state = 4
            logoB.z = 1.125
            logoC.z = 1.75
        end
    elseif state == 4 then
        logoC.z = logoC.z - 0.025
        if logoC.z <= 1.25 then
            state = 5
            logoC.z = 1.25
            make_title()
        end
    elseif state >= 5 then
        state = state + 1
        if state > 200 then
            quit(1)
        end
    end


    -- logo.rz = logo.rz + 0.02
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
end

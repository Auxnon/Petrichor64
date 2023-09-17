-- Codex 3.0.0 "Artichoke"
sky:fill('FF5')
camo = { x = 0, y = 0, z = 0 }
rotat = tau / 4

function main()
    -- mod("")
    local s = 4
    plaid = nimg(s, s)
    for i = 1, s do
        for j = 1, s do
            if (i + j) % 2 == 0 then
                plaid:pixel(i - 1, j - 1, '000')
            else
                plaid:pixel(i - 1, j - 1, 'F0F')
            end
        end
    end



    tex('plaid', plaid)
    -- generated = make('plane', 0, 12)
    -- generated.tex = "plaid"
    -- -- generated.
    -- generated.scale = 10
    -- generated.rx = tau / 8
    -- generated.ry = tau / 16
    spin = 0

    function thingy(x, y)
        local t = make('plane', x, y)
        t.tex = 'plaid'
        t.scale = 10
        return t
    end

    for i = 0, 12 do
        local n = thingy(-10, i * 10)
        n.rx = tau / 4
        n.ry = tau / 4
        n = n:copy()
        n.z = 10

        n = thingy(10, i * 10)
        n.rx = tau / 4
        n.ry = tau / 4

        n = n:copy()
        n.z = 10
        -- n = thingy(0, i)
        -- t.ry = tau / 16
        -- t.x = rnd(-10, 10)
        -- t.z = rnd(-10, 10)
        -- t.y = rnd(0, 10)
    end

    -- for i = 0, 6 do
    --     for j = 0, 6 do
    --         local t = 'example'
    --         if (j + i + 1) % 2 == 0 then
    --             t = 'generated'
    --         end
    --         tile(t, i - 3, 9 + j, -3)
    --     end
    -- end

    cout 'main runs once everything has loaded'
end

function loop()
    -- example.x = example.x + rnd(-0.05, 0.05)
    -- example.z = example.z + rnd(-0.05, 0.05)
    -- spin = spin + 0.1
    -- generated.z = generated.z + cos(spin) * .04
    -- generated.x = generated.x + sin(spin) * .04
    if key("w") then
        camo.y = camo.y + .1
    end
    if key("s") then
        camo.y = camo.y - .1
    end

    if key("a") then
        rotat = rotat + .01
    end
    if key("d") then
        rotat = rotat - .01
    end

    cam { pos = { camo.x, camo.y, camo.z }, rot = { rotat, 0 } }
end

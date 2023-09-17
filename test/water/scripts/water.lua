-- Codex 3.0.0 "Artichoke"
-- attr { fog = 800, modernize = 0, resolution = 200 }
attr { fog = 800 }
sky:fill('5fcde4')

function waver(i)
    w:fill('04A')
    w:img(under, i * 2, 0)
    w:img(under, 32 + i * 2, 0)
    w:img(ripple, i * 2, i * 2)
    w:img(ripple, -32 + i * 2, i * 2)
    w:img(ripple, -32 + i * 2, -32 + i * 2)
    w:img(ripple, i * 2, -32 + i * 2)

    tex("wave1", w)
end

function main()
    w = nimg(32, 32)
    w:fill('04A')
    ripple = gimg("water")
    under = ripple:copy()
    under:fill('000', '5fcde4')
    wmove = 0
    wdelay = 0
    tex("wave1", w)
    -- local a = {}
    -- for i = 1, 5 do
    --     local w1 = w:copy()
    --     w1:img(ripple, i * 2, i * 2)
    --     w1:img(under, i * 3, 0)
    --     local n = 'wave' .. i
    --     a[i] = n
    --     tex(n, w1)
    -- end
    -- anim("wave", a, 8)

    example = make('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)

    local im = nimg(16, 16)
    im:fill('CC8')
    tex('generated', im)
    generated = make('generated', 0, 9, -1.1)
    generated.scale = .5
    -- generated:anim("wave")
    spin = 0

    for i = -30, 30 do
        for j = 4, 69 do
            local t = 'example'
            if (j + i + 1) % 2 == 0 then
                t = 'generated'
            end
            tile("wave1", i, j, -2)
        end
    end

    cam { pos = { 0, 0, -.5 }, rot = { tau / 4, 0 } }
    cout 'main runs once everything has loaded'
end

mover = 0
function loop()
    wdelay = wdelay + 1
    if wdelay > 32 then
        wdelay = 0
        waver(wmove)
        wmove = wmove + 1
        if wmove > 16 then
            wmove = 0
        end
    end
    -- example.x = example.x + rnd(-0.05, 0.05)
    -- example.z = example.z + rnd(-0.05, 0.05)
    spin = spin + 0.1
    -- generated.z = generated.z + cos(spin) * .04
    generated.x = generated.x + .002
    -- mover = mover + 0.01
    cam { pos = { mover, mover, -.5 } }
end

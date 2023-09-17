-- Codex 3.0.0 "Artichoke"
sky:fill('FF5')

function main()
    local im = nimg(16, 16)
    im:fill('00F')
    tex('generated', im)
    -- example = make('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
    mod("panel", { q = { { .4, 0, 1 }, { .6, 0, 1 }, { .6, 0, 0 }, { .4, 0, 0 } }, t = { "generated" } })
    mod("bod", { q = { { 0, 0, 1 }, { 1, 0, 1 }, { 1, 0, .8 }, { 0, 0, .8 } }, t = { "generated" } })


    generated = make('bod', 0, 0, 0)
    leg1 = make("panel", .4, 0.4, 0)
    leg2 = make("panel", -.4, 0.4, 0)
    leg3 = make("panel", .4, -0.4, 0)
    leg4 = make("panel", -.4, -0.4, 0)
    lot(generated, leg1)
    lot(generated, leg2)
    lot(generated, leg3)
    lot(generated, leg4)
    generated.y = 12
    spin = 0

    for i = 0, 6 do
        for j = 0, 6 do
            local t = 'example'
            if (j + i + 1) % 2 == 0 then
                t = 'generated'
            end
            tile(t, i - 3, 9 + j, -3)
        end
    end

    cam { pos = { 0, 0, 0 }, rot = { tau / 4, 0 } }
    cout 'main runs once everything has loaded'
end

function loop()
    -- example.x = example.x + rnd(-0.05, 0.05)
    -- example.z = example.z + rnd(-0.05, 0.05)
    spin = spin + 0.1
    generated.z = generated.z + cos(spin) * .04
    generated.x = generated.x + sin(spin) * .04
    -- generated.rz = spin / 4
    leg1.z = cos(spin) * .4
    leg1.x = sin(spin) * .2 + .4
    leg4.z = cos(spin) * .4
    leg4.x = sin(spin) * .2 - .4
    leg2.z = cos(spin + tau / 2) * .2
    leg2.x = sin(spin + tau / 2) * .2 + .4
    leg3.z = cos(spin + tau / 2) * .2
    leg3.x = sin(spin + tau / 2) * .2 - .4
end

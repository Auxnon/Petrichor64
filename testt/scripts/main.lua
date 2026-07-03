-- Codex 3.0.0 "Artichoke"
sky:fill('FF5')

function main()

    local im = nimg(16, 16)
    im:fill('00F')
    im:line(0,0,10,10,'F0F')
    tex('generated', im)
    example = make('example', 12,rnd() * 3. - 1.5,  rnd() * 3. - 1.5)
    generated = make('generated',10,  rnd() * 3. - 1.5,  rnd() * 3. - 1.5)
    spin = 0

    for i = 0, 6, 1 do
        for j = 0, 6, 1 do
            local t = 'example'
            if (j + i + 1) % 2 == 0 then
                t = 'generated'
            end
            tile(t,  9 + j,i - 3, -3)
        end
    end

    -- cam { pos = { 0, 0, 0 }, rot = { tau / 4, 0 } }
    cout 'main runs once everything has loaded'
end

function loop()
    print(tau)
    example.y = example.y + rnd(-0.05, 0.05)
    example.z = example.z + rnd(-0.05, 0.05)
    spin = spin + 0.1
    generated.z = generated.z + cos(spin) * .04
    generated.y = generated.y + sin(spin) * .04
    cam { pos = { cos(spin), 0, 0 }, rot = { 0, 0 } }
end

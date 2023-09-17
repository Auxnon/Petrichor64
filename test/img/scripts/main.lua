-- Codex 3.0.0 "Artichoke"
sky:fill('FF5')

function main()
    for i = 0, 20 do
        sky:img(gimg("example"), rnd(), rnd())
    end
    -- example = make('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)

    -- local im = nimg(16, 16)
    -- im:fill('00F')
    -- tex('generated', im)
    -- generated = make('generated', rnd() * 3. - 1.5, 10, rnd() * 3. - 1.5)
    -- spin = 0

    -- for i = 0, 6 do
    --     for j = 0, 6 do
    --         local t = 'example'
    --         if (j + i + 1) % 2 == 0 then
    --             t = 'generated'
    --         end
    --         tile(t, i - 3, 9 + j, -3)
    --     end
    -- end

    -- cam { pos = { 0, 0, 0 }, rot = { tau / 4, 0 } }
    -- cout 'main runs once everything has loaded'
end

-- function loop()
--     example.x = example.x + rnd(-0.05, 0.05)
--     example.z = example.z + rnd(-0.05, 0.05)
--     spin = spin + 0.1
--     generated.z = generated.z + cos(spin) * .04
--     generated.x = generated.x + sin(spin) * .04
-- end

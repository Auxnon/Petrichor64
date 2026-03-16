-- Codex 3.0.0 "Artichoke"
print "we in file"
sky:fill('FF5')

function main()
    print "main fn in file"
    example = make('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)

    cam { pos = { 0, 0, 0 }, rot = { tau / 4, 0 } }
    -- cout 'main runs once everything has loaded'
end

function loop()
    example.x = example.x + rnd(-0.05, 0.05)
    example.z = example.z + rnd(-0.05, 0.05)
end

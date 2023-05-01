-- Codex 3.0.0 "Artichoke"
sky:fill('FF5')
function main()
    local one = nimg(16, 16)
    one:fill('F00')
    tex("one", one)
    local two = nimg(16, 16)
    two:fill('0F0')
    tex("two", two)
    anim("blink", { "one", "two" })

    example = make('blink', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
    example:anim("blink")
    cout 'main runs once everything has loaded'
end

function loop()
    example.x = example.x + rnd() * 0.1 - 0.05
    example.z = example.z + rnd() * 0.1 - 0.05
end

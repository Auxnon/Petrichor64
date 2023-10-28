-- Codex 3.0.0 "Artichoke"
sky:fill('303')
attr { modernize = 0, glitch = { .5, .12 }, resolution = 720 }

function main()
    t = 0
    local m = nimg(72, 32)
    m:text('no signal', 0, 8)
    tex('m', m)
    e = make('m', 0, 5, 0)
    cam { pos = { 0, 0, 0 }, rot = { tau / 4, 0 } }
end

function loop()
    t = t + 0.08
    if t > tau then
        t = 0
    end
    e.x = cos(t) * .1
end

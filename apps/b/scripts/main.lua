attr { modernize = 0,
    dark = .8,
    glitch = { 0.98, .23 },
    resolution = 128,
    curvature = 0.9,
    high = 0.6,
    low = 0.05,
    bleed = 0.,
    lock = true

}
b = spawn('b', -.4, 80, -30.)
b.scale = 6
sky()
fill("fff")
gui()
g = 0
gdir = 1
sp = 0.02
function main()
    log('main runs once everything has loaded')
end

function loop()
    g = g + 0.002 * gdir
    if g > 1 then gdir = -1 elseif g < 0 then gdir = 1 end
    attr { glitch = { 0.98, .23 } } --.985
    -- log('loop runs every frame')
    b.y = b.y - sp
    b.z = b.z + sp * 3.15 / 8
    if b.y <= 0 then
        quit()
    end
    print(g)
    -- clr()
    -- rect(0, 0, 8, g, "f00")
    -- text(flr(g * 100.) / 100.)
    -- example.z = example.z + rnd() * 0.1 - 0.05
end

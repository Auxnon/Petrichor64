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
b = make('b', -.4, 80, -30.)
b.scale = 6
sky:fill("fff")
g = 0
gdir = 1
sp = 0.02
cam { rot = { pi / 2, 0 } }

function loop()
    b.y = b.y - sp
    if b.y <= 0 then
    elseif b.y <= 20 and b.y > 8 then
        b.y = 8
        b.z = -0.5
        attr { glitch = { 0.99, 300. }, dark = 0.2 }
    elseif b.y < 7 then
        quit()
    else
        b.z = b.z + sp * 3.15 / 8
    end
end

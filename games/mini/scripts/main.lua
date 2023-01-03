example = spawn('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
sky()
fill(1, 1, .4, 1)
gui()
t = 0
camera = { x = 0, y = 0, z = 0 }


function land(h)
    for i = -5, 5 do
        for j = 0, 10 do
            tile("blocks0", i, j + 10, h - 1)
            tile("blocks1", i, j + 10, h)
        end
    end
    for n = 1, 20 do
        tree(n - 5, 10 + n, h)
        -- tree(irnd(10) - 5, irnd(10) + 10, h)
    end
    tile("example", 0, 12, h + 1)
end

function irnd(a)
    return flr(rnd() * a)
end

function tree(x, y, h)
    tile("trunk", x, y, h + 1)
    tile("blocks8", x, y, h + 2)
    -- print("tr" .. x .. "," .. y .. "," .. h)
end

function main()
    init_test()
    log('main runs once everything has loaded')
    land(-2)

    camrot(pi / 2., 0) -- -pi / 8.)

end

tiler = 1
function loop()
    example.x = example.x + rnd() * 0.1 - 0.05
    example.z = example.z + rnd() * 0.1 - 0.05
    if key("space", true) then
        -- - 5, 10
        tile("blocks" .. tiler, tiler - 5, tiler + 10, 0)
        text("t" .. tiler, tiler * 10)
        tiler = 1 + tiler
    end
    t = t + 0.2
    -- camrot(pi / 2., t)

    -- camrot(0, t)
    loop_controls()
    loop_test()
    campos(camera.x, camera.y, camera.z)
end

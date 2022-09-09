math.randomseed(os.time())
crt({
    dark = 99.,
    low = 0.1,
    high = 0.9,
    flatness = 4.,
    curvature = 0.8,
    resolution = 720,
    glitch = 4.
})

bg(0.1, 1., 1.)
function rnd(min, max)
    if max == nil then
        max = min
        min = 0
    end
    return min + math.random() * (max - min)
end

player = {}
remote = {}
function main()

    player = {
        h = rnd(10.),
        ele = spawn("birdo", 4., -1., -0.9),
        vel = 0.
    }

    remote = {
        h = rnd(10.),
        ele = spawn("birdo", 4., 1., -0.9),
        vel = 0.
    }

    for i = 1, 50 do
        spawn("poofy", 18, rnd(-12., 12.), rnd(1., 8.))
    end
    for i = -40, 40 do
        for j = -1, 40 do
            h = math.random()
            ro = 0
            if h > 0.8 then
                t = "E"
                ro = 1
            elseif h > 0.6 then
                t = "F"
                ro = 2
            elseif h > 0.4 then
                t = "U"
                ro = 3
            elseif h > 0.2 then
                t = "L"
                ro = 2
            else
                t = "S"
            end
            tile("grass-block.U", j * 2, i, -2, ro)

            -- tile("beveled_cube.block", j * 2, i * 2, 0)

            -- tile("beveled_cube.torch", j * 2, i * 2, 1) -- 9 10 17 18
        end
    end

end

camera = {
    x = 0,
    y = 0,
    z = 0
}

function loop()
    local x = player.ele.x
    local y = player.ele.y
    local speed = 0.05

    if key("w") then
        x = x + speed
    elseif key("s") then
        x = x - speed
    end

    if key("a") then
        y = y + speed
    elseif key("d") then
        y = y - speed
    end

    if player.ele.x ~= x or player.ele.y ~= y then
        player.ele.x = x
        player.ele.y = y
        send(player.ele.x, player.ele.y, player.ele.z)
    end

    p = recv()

    v = {
        x = p[1],
        y = p[2],
        z = p[3]
    }

    if v.x ~= 0 then
        remote.ele.x = v.x
        remote.ele.y = v.y
        remote.ele.z = v.z
    end

    campos(camera.x, camera.y, camera.z)

end

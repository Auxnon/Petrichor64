example = spawn('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
player = { x = 0, y = 0, z = 0, vx = 0, vy = 0, vz = 0 }
cam = { x = 0, y = 0, z = 0 }
rot = { x = 0, y = 0 }
srot = { x = 0, y = 0 }
m = { x = 0, y = 0 }
bg(1, 1, .4, 1)
attr { mouse_grab = 1, fullscreen = 1 }
absolute_speed = 0

aim = spawn("square", 0, 0, 0)

the_size = 6
ni = -the_size
nj = -the_size
fire_delay = 0
bullets = {}
-- for ni = -12, 12 do
--     local ii = ni * 40
--     for nj = -12, 12 do
--         local jj = nj * 40

--     end
-- end

function makey()
    if ni < the_size then
        local ii = ni * 80
        for nj = -the_size, the_size do
            local jj = nj * 80
            doot(ii, jj)
        end
        ni = ni + 1
    end
end

function doot(ii, jj)
    for i = -10, 10 do
        for j = -10, 10 do
            -- local m = 10 - (abs(i) + abs(j)) / 3
            tile("square", i + ii, j + jj, -2)
            -- tile("square", m + i + ii, j + jj, m)
        end
    end

    for zz = 6, 34, 4 do
        for i = -10, 10 do
            tile("square", i + ii, -10 + jj, zz + rnd() * 10)
            tile("square", i + ii, 10 + jj, zz + rnd() * 10)
        end

        for i = -9, 9 do
            tile("square", -10 + ii, i + jj, zz + rnd() * 10)
            tile("square", 10 + ii, i + jj, zz + rnd() * 10)
        end
    end


    -- tile("grid", i + ii, j + jj, 6)
end

doot(0, 0)

function clamp(n, min, max)
    if n < min then
        return min
    elseif n > max then
        return max
    else
        return n
    end
end

spots = {}
function make_thing()

    spots[#spots + 1] = spawn("example", player.x, player.y, player.z - 2)
    if #spots > 120 then
        -- for i = 1, #spots - 10 do

        -- spots[1].kill()

        -- end
        table.remove(spots, 1):kill()
        -- spots=table.slice(spots,#spots-10,#spots)
    end
end

function main()
    log('main runs once everything has loaded')
end

function speedcap()
    absolute_speed = sqrt(player.vx * player.vx + player.vy * player.vy + player.vz * player.vz)
    local speed = 0.2
    if key("lshift") then speed = 2. end
    if absolute_speed > speed then
        player.vx = speed * player.vx / absolute_speed
        player.vy = speed * player.vy / absolute_speed
        player.vz = speed * player.vz / absolute_speed
    end
end

function loop()
    makey()
    example.x = example.x + rnd() * 0.1 - 0.05
    example.z = example.z + rnd() * 0.1 - 0.05
    local mp = m
    m = mouse()
    local ang = 0
    local azi = 0
    ang = m.dx
    azi = m.dy

    --
    rot.x = rot.x - ang / 240.
    rot.y = rot.y - azi / 240.

    rot.y = clamp(rot.y, -pi / 2 + 0.0001, pi / 2 - 0.0001)

    srot.x = rot.x --srot.x + (rot.x - srot.x) / 2.
    srot.y = rot.y --srot.y + (rot.y - srot.y) / 2.

    -- if key('return', true) then
    --     print "return"
    --     attr { fullscreen = 0 }
    -- end
    if key('w') then
        player.vx = player.vx + math.cos(srot.x) * 0.1
        player.vy = player.vy + math.sin(srot.x) * 0.1
        speedcap()
    elseif key('s') then
        player.vx = player.vx - math.cos(srot.x) * 0.1
        player.vy = player.vy - math.sin(srot.x) * 0.1
        speedcap()
    end

    if key('a') then
        player.vx = player.vx + math.cos(srot.x + math.pi / 2) * 0.1
        player.vy = player.vy + math.sin(srot.x + math.pi / 2) * 0.1
        speedcap()
    elseif key('d') then
        player.vx = player.vx - math.cos(srot.x + math.pi / 2) * 0.1
        player.vy = player.vy - math.sin(srot.x + math.pi / 2) * 0.1
        speedcap()

    end

    player.x = player.x + player.vx
    player.y = player.y + player.vy
    player.z = player.z + player.vz

    local friction = 0.9
    if absolute_speed > 0.7 then
        friction = 0.95
    end
    player.vx = player.vx * friction
    player.vy = player.vy * friction
    player.vz = player.vz * friction

    cam.x = cam.x + (player.x - cam.x) / 2.
    cam.y = cam.y + (player.y - cam.y) / 2.
    cam.z = cam.z + (player.z - cam.z) / 2.

    make_thing()

    aim.z = player.z + math.sin(srot.y) * 10
    local xz = math.cos(srot.y) * 10
    aim.x = player.x + math.cos(srot.x) * xz
    aim.y = player.y + math.sin(srot.x) * xz

    if fire_delay <= 0 then
        fire_delay = 3
        local b = spawn("example", player.x, player.y, player.z)
        local bb = { ent = b,
            vx = aim.x - player.x,
            vy = aim.y - player.y,
            vz = aim.z - player.z,
        }
        local speed = 1
        bb.vx = bb.vx * speed
        bb.vy = bb.vy * speed
        bb.vz = bb.vz * speed
        table.insert(bullets, bb)
    else
        fire_delay = fire_delay - 1
    end

    for i = #bullets, 1, -1 do
        local b = bullets[i]
        b.ent.x = b.ent.x + b.vx
        b.ent.y = b.ent.y + b.vy
        b.ent.z = b.ent.z + b.vz
        b.vz = b.vz - 0.01
        if b.ent.z < -2 then
            b.ent:kill()
            table.remove(bullets, i)
        end
        -- return b.ent.z > 0
    end



    camrot(srot.x, srot.y)
    campos(cam.x, cam.y, cam.z)
end

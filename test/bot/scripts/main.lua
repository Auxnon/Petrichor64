example = spawn('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
player = { x = 0, y = 0, z = -2, vx = 0, vy = 0, vz = 0 }
cm = { x = 0, y = 0, z = 0 }
rot = { x = 0, y = 0 }
srot = { x = 0, y = 0 }
m = { x = 0, y = 0 }

attr { mouse_grab = true } --fullscreen = 1
absolute_speed = 0

aim = spawn("screen", 0, 0, 0)
sqimg = gimg("screen")
rimg = gimg("red")
ogimg = sqimg:copy()
ogrimg = rimg:copy()

the_size = 7
ni = -the_size
nj = -the_size
fire_delay = 0
bullets = {}
bot = spawn("bot.bot", 0, 0, 0)
-- bot.scale = 2
m_off = false
pheight = 6


function model_test()
    -- local n = 0.5
    -- -- pyramid
    -- local v = { { 0, 0, 0 }, { n, 0, 0 }, { n, n, 0 }, { 0, n, 0 }, { 0, n, n }, { 0, 0, n } }
    -- local i = { 0, 1, 2, 0, 2, 3, 0, 3, 4, 0, 4, 5, 1, 2, 5, 2, 4, 5, 2, 3, 4, 1, 3, 5 }
    -- local u = { { 0, 0 }, { 1, 0 }, { 1, 1 }, { 0, 1 }, { 0, 0 }, { 1, 0 } }
    -- -- local v = { { -n, -n, 0 }, { n, -n, 0 }, { n, n, 0 }, { -n, n, 0 } }
    -- smodel("thing",
    --     { t = "square", v = v, i = i, u = u })


    model("thing",
        { t = { "square" }, v = { { -0.8, -1, 0 }, { 0.8, -1, 0 }, { 0.8, 1, 0 }, { -0.8, 1, 0 } },
            i = { 0, 1, 2, 0, 2, 3 },
            uv = { { 0, 0 }, { 1, 0 }, { 1, 1 }, { 0, 1 } } })
end

model_test()

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
            tile("thing", i + ii, j + jj, -2)
            -- tile("square", m + i + ii, j + jj, m)
        end
    end

    for zz = 6, 34, 4 do
        for i = -10, 10 do
            tile("screen", i + ii, -10 + jj, zz)
            tile("screen", i + ii, 10 + jj, zz)
        end

        for i = -9, 9 do
            tile("screen", -10 + ii, i + jj, zz)
            tile("screen", 10 + ii, i + jj, zz)
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
    local e = spawn("smoke", player.x, player.y, player.z - 2)
    e.vx = rnd() * 0.05 - 0.025
    e.vy = rnd() * 0.05 - 0.025
    e.vz = rnd() * 0.05 - 0.025
    spots[#spots + 1] = e
    if #spots > 200 then
        -- for i = 1, #spots - 10 do

        -- spots[1].kill()

        -- end
        table.remove(spots, 1):kill()
        -- :kill()
        -- spots=table.slice(spots,#spots-10,#spots)
    end
end

function move_things()
    for i = 1, #spots do
        local s = spots[i]
        s.x = s.x + s.vx
        s.y = s.y + s.vy
        s.z = s.z + s.vz
    end

end

function main()
    log('main runs once everything has loaded')
    sky()
    fill("fff")
    gui()
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

function place(a, b)
    a.x = b.x
    a.y = b.y
    a.z = b.z
end

function rnda()
    return rnd() * 0.4 + 0.6
end

function rndb()
    return rnd() - 0.5
end

function fire()
    local b = spawn("red", player.x, player.y, player.z + pheight)
    -- tile("red", player.x, player.y, player.z + pheight)
    -- local affect = 0.1
    local bb = { ent = b,
        vx = (aim.x - player.x) + rndb(),
        vy = (aim.y - player.y) + rndb(),
        vz = (aim.z - player.z) + rndb(),
        t = 100
    }
    local speed = 0.7
    bb.vx = bb.vx * speed * rnda() + player.vx
    bb.vy = bb.vy * speed * rnda() + player.vy
    bb.vz = bb.vz * speed * rnda() + player.vz
    local r = rnd();
    bb.ent.x = bb.ent.x + bb.vx * r
    bb.ent.y = bb.ent.y + bb.vy * r
    bb.ent.z = bb.ent.z + bb.vz * r

    table.insert(bullets, bb)
end

county = 0
scounty = 0
function imgey()
    scounty = scounty + 1
    if scounty > 60 then
        county = county + 1
        scounty = 0
        sqimg:dimg(ogimg)
        rimg:clr()
        rimg:dimg(ogrimg)
        -- sqimg:line(rnd(), rnd(), rnd(), rnd())
        sqimg:text("" .. county, 0, 12)
        if county % 2 == 0 then
            rimg:text("A", -1, 4)
        else
            rimg:text("B", -1, 4)
        end
        tex("screen", sqimg)
        tex("red", rimg)
    end

end

function loop()
    makey()
    imgey()
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

    if istile(player.x, player.y, player.z) then
        player.vz = player.vz + 0.02
    elseif player.z > 0 then
        player.vz = player.vz - 0.02
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
    place(bot, player)
    bot.rz = srot.x + pi / 2.




    cm.x = player.x - math.cos(srot.x) * 12
    cm.y = player.y - math.sin(srot.x) * 12
    cm.z = player.z + pheight

    make_thing()
    move_things()


    aim.z = player.z + math.sin(srot.y) * 4
    local xz = math.cos(srot.y) * 4
    aim.x = player.x + math.cos(srot.x) * xz
    aim.y = player.y + math.sin(srot.x) * xz

    if m.m1 then
        if not m_off then
            for i = 1, 100 do
                fire()
            end
        end
        m_off = true

    else
        m_off = false
    end


    -- if fire_delay <= 0 then
    -- fire_delay = 1

    -- else
    --     fire_delay = fire_delay - 1
    -- end

    aim.z = aim.z + pheight


    for i = #bullets, 1, -1 do
        local b = bullets[i]
        b.ent.x = b.ent.x + b.vx
        b.ent.y = b.ent.y + b.vy
        b.ent.z = b.ent.z + b.vz
        b.vz = b.vz - 0.01
        b.t = b.t - 1
        if b.t < 0 then
            b.ent:kill()
            table.remove(bullets, i)
            if not istile(b.ent.x, b.ent.y, b.ent.z) then
                tile("red", b.ent.x, b.ent.y, b.ent.z)
            end
        elseif istile(b.ent.x, b.ent.y, b.ent.z) then
            b.ent:kill()
            table.remove(bullets, i)
            tile("red", b.ent.x - b.vx, b.ent.y - b.vy, b.ent.z - b.vz)


        elseif b.ent.z < -2 then
            b.ent:kill()
            table.remove(bullets, i)
            if not istile(b.ent.x, b.ent.y, b.ent.z) then
                tile("red", b.ent.x, b.ent.y, b.ent.z)
            end
        end
        -- return b.ent.z > 0
    end



    cam { pos = { cm.x, cm.y, cm.z }, rot = { srot.x, srot.y } }


    clr()
    text("x " .. flr(player.x * 100) / 100, 0, 30) --s.. "  y " .. flr(player.y * 100))
end

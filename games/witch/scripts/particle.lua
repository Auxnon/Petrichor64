particles = {}
missile_delay = 0

function Particle_Init()
    local sprk = {}
    for i = 0, 7 do sprk[i + 1] = "sparkles" .. i end
    anim("sparkler", sprk, 2)
    Missiles = Dict:new()
    local leaves = batch_spawn(100, "leaf", 0, 12, 4, 50)
    for i = 1, #leaves do
        local p = leaves[i]
        p.x = (rnd() * 20) - 10
        local leaf = {
            ent = p,
            vx = rnd() * 0.05,
            vy = 0,
            vz = -rnd() * 0.05
        }
        particles[#particles + 1] = leaf
    end
    -- spark = spawn("spark", 0, 0, 0)
    -- for i = 1, 10 do
    --     local s = spawn("spark", i, 0, 0)
    --     sparks[#sparks + 1] = s
    -- end
end

function make_missile(target)
    if target then
        local sparkles = batch_spawn(10, "sparkles1", player.x, player.y, player.z)
        for i = 1, #sparkles do
            sparkles[i]:anim("sparkler", i)
        end

        local m = {
            state = 0,
            sparkles = sparkles,
            target = target,
            current = { x = player.x, y = player.y, z = player.z + 2 },
            delay = 0
        }

        Missiles:add(m)
    end
end

function kill_missile(m)
    for i = 1, #m.sparkles do
        kill(m.sparkles[i])
    end
end

function move_missile(m)
    -- log("move to " .. m.sparkles[1].x .. " 2 " .. m.current.x)
    local r = orbit(m.sparkles[1], m.current, 0.1)
    for s = 2, #m.sparkles do
        follow(m.sparkles[s], m.sparkles[s - 1], 2., 0.)
    end
    return r

end

function orbit(a, b, d)
    local vx = (a.x - b.x) + 0.00001
    local vy = (a.y - b.y) + 0.00001
    local vz = (a.z - b.z) + 0.00001

    local rr = sqrt(vx * vx + vy * vy + vz * vz)
    local r = d / rr

    a.x = a.x - vx * r
    a.y = a.y - vy * r
    a.z = a.z - vz * r
    return rr
end

function follow(a, b, d, o)
    local vx = a.x - b.x
    local vy = a.y - b.y
    local vz = a.z - (b.z + o)
    a.x = a.x - vx / d
    a.y = a.y - vy / d
    a.z = a.z - vz / d
end

function move(a, b, c)
    c = c or 0
    a.x = b.x
    a.y = b.y
    a.z = b.z + c
end

function Particle_Loop()
    for i = 1, #particles do
        local l = particles[i]
        l.ent.x = l.ent.x + l.vx
        l.ent.z = l.ent.z + l.vz
        if l.ent.z < -2 then
            l.ent.z = 12
            l.ent.x = rnd() * 40 - 20
            l.ent.y = rnd() * 20
        end
    end

    if key("space") and missile_delay == 0 then
        local zlist = Zoms:list()
        local cap = nil
        local shortest = 10
        for i = 1, #zlist do
            local z = zlist[i]
            local dist = abs(z.ent.x - player.x)
            if dist < shortest then
                shortest = dist
                cap = z
            end
        end

        if cap then
            missile_delay = 30

            make_missile(cap)
        end
    end

    if missile_delay > 0 then
        missile_delay = missile_delay - 1
    end

    local mlist = Missiles:list()
    for i = 1, #mlist do
        local m = mlist[i]
        m.delay = m.delay + 1
        if m.delay > 30 then
            m.current = m.target.ent
            if move_missile(m) < 0.5 then

                -- print("target was " .. m.target.id)
                -- print("is " .. type(Zoms:get(m.target.id)) .. " size " .. #Zoms:list())
                Zoms:remove(m.target)
                kill(m.target.ent)

                print("this far")
                Missiles:remove(m)
                kill_missile(m)
                print("done")
                -- print("is " .. type(Zoms:get(m.target.id)) .. " size " .. #Zoms:list())
                i = i - 1
            end
        else
            move_missile(m)
        end



    end
    -- if spark_delay > 0 then
    --     spark_delay = spark_delay - 1
    --     g = {
    --         x = player.x,
    --         y = player.y,
    --         z = player.z + 4
    --     }
    -- else
    --     g = spark_target
    -- end

    -- orbit(spark, g, 0.5)
    -- follow(sparks[1], spark, 2., 0.)
    -- for i = 2, #sparks do
    --     follow(sparks[i], sparks[i - 1], 2., cos(sparks[i - 1].x) * .2)
    -- end

end

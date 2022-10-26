Zoms = {}
Guys = {}
function zombie()
    local z = spawn("zom", -DISTANCE + (rnd() * 6) - 9, 12, -0.5)
    z:anim("zom.rise")
    local e = { delay = 90, ent = z }
    Zoms:add(e)
end

function guy()
    local z = spawn("guy", -DISTANCE + (rnd() * 6) - 9, 12, -0.5)
    z:anim("guy.walk")
    local e = { delay = 90, ent = z }
    Guys:add(e)
end

function enemy_start()
    -- zoom = spawn("zom", 2 * DISTANCE, 0, -0.5)
    Zoms = Dict:new()
    Guys = Dict:new()
end

local delay = 0
function enemy_loop()
    -- zoom.y = zoom.y + 0.02
    if delay > 120 then
        guy()
        delay = 0
    end

    local zlist = Zoms:list()
    -- print(#zlist)
    for i = 1, #zlist do
        local z = zlist[i]
        local e = z.ent


        if z.delay > 0 then
            z.delay = z.delay - 1
            if z.delay == 0 then
                e:anim("zom.walk")
            end
        else

            local dist = abs(e.x - player.x)
            if dist < 0.75 then
                e:anim("zom.attack")
            else
                e.x = e.x + 0.01
                e:anim("zom.walk")
            end
            -- yeh
            if e.x > 12 then
                e.x = -DISTANCE
                z.delay = 90
                e:anim("zom.rise")
            end

        end
    end

    delay = delay + 1

end

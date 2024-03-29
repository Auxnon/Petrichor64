Zoms = {}
Guys = {}
Tools = {}
Fires = {}



function zombie()
    if not ZOM_BAN then
        local z = spawn("zom", -DISTANCE + (rnd() * 6) - 9, 12, -0.5)
        z:anim("zom.rise")
        local e = { delay = 90, ent = z }
        Zoms:add(e)
    end
end

function guy()
    if not GUY_STINK then
        local z = spawn("guy", DISTANCE * 2, 0, -0.5)
        z.flipped = true
        z:anim("guy.walk")

        local tool = spawn("mob-tools" .. flr(rnd() * 3), DISTANCE * 2, 0, -0.5)

        local e = { delay = 90, ent = z, fighting = false, stopat = 8 + (rnd() * 4), tool = tool }
        Guys:add(e)
    end

end

function fires()
    local z = spawn("fire", 0, MID, -10)
    z:anim("fire.fire")
    -- Fires:add(z)
    return z
end

function enemy_start()
    -- zoom = spawn("zom", 2 * DISTANCE, 0, -0.5)
    Zoms = Dict:new()
    Guys = Dict:new()
    Fires = {}
    for i = 1, 10 do
        Fires[i] = fires()
    end
    BURNING = false
    BURN_DELAY = 100
    TORCHED = false
end

local delay = 0
local gdelay = 0
function enemy_loop()
    -- zoom.y = zoom.y + 0.02
    if delay > ZOM_RATE then
        zombie()
        delay = 0
    end

    if gdelay > GUY_RATE then
        guy()
        gdelay = 0
    end

    zom_loop()

    guy_loop()
    delay = delay + 1
    gdelay = gdelay + 1

end

function zom_loop()
    local zlist = Zoms:list()
    for i = 1, #zlist do
        local z = zlist[i]
        local e = z.ent


        if z.delay > 0 then
            z.delay = z.delay - 1
            if z.delay == 0 then
                e:anim("zom.walk")
            end
        else
            local go = true
            if LEFT_DOOR then
                local dist = abs(e.x - (-6))
                if dist < 0.75 then
                    go = false
                    e:anim("zom.attack")
                    LDOOR_HP = LDOOR_HP - 0.05
                end
            else
                local dist = abs(e.x - (-1))
                if dist < 0.75 then
                    go = false
                    e:anim("zom.attack")
                    POT_HP = POT_HP - 0.05
                end
            end

            if go then
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
end

function guy_loop()
    local glist = Guys:list()
    local fighters = 0
    for i = 1, #glist do
        local g = glist[i]
        local e = g.ent
        if g.fighting then
            fighters = fighters + 1
            if BURNING and (not TORCHED) and i <= 10 then
                local f = Fires[i]
                -- if not f then
                --     f = fires()
                --     Fires[i] = f
                -- end
                move(f, e, 1)
            end
        else
            if GUY_STINK then
                e.flipped = false
            else
                e.flipped = true
            end
            if e.y < MID then
                if GUY_STINK then
                    e.y = e.y - 0.03
                else
                    e.y = e.y + 0.015
                end
            elseif e.x > g.stopat then
                if GUY_STINK then
                    e.x = e.x + 0.02
                else
                    e.x = e.x - 0.01
                end
            elseif not g.fighting then
                g.fighting = true
                e:anim("guy.angry")
            elseif GUY_STINK then
                e:anim("guy.walk")
                e.x = e.x + 0.02
            end

            if g.tool then
                g.tool.x = e.x - 0.375
                g.tool.y = e.y + 0.1
                g.tool.z = e.z
                g.tool.rot_z = g.tool.rot_z + 0.1
                g.tool.rot_y = g.tool.rot_y + 0.1
                g.tool.rot_x = g.tool.rot_x + 0.1
            end
        end
    end

    local changed_burn = BURNING
    BURNING = fighters > 10

    if changed_burn and not BURNING then
        for i = 1, 10 do
            Fires[i].z = -10
        end
    end

    if not BURNING then
        BURN_DELAY = 600
    end

    if BURNING then
        if BURN_DELAY > 0 then
            BURN_DELAY = BURN_DELAY - 1
            if BURN_DELAY <= 0 then
                for i = 1, 10 do
                    Fires[i].x = rnd() * 16 - 8
                    Fires[i].z = rnd() * 12
                end
                lose()
            end
        else
            TORCHED = true
        end
    end

end

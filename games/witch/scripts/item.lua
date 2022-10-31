function make_bottle(x, z, stinky)
    local n = 0
    if stinky then n = 1 end
    local b = spawn("potions" .. n, x, MID, z - 0.5)
    local bb = { ent = b, vz = 0, carried = false, active = false, stinky = stinky }
    items:add(bb)
end

function make_shroom(x, z)
    local type = shroom_order[shroom_counter]
    local b = spawn("mushroom" .. type, x, MID, z - 0.5)
    local bb = { ent = b, vz = 0, carried = false, active = false, type = type }
    items:add(bb)
    shroom_counter = shroom_counter + 1
    if shroom_counter > #shroom_order then
        shroom_counter = 1
    end
end

local zready = true

function init_items()
    items = Dict:new()
    carry = Dict:new()
    CARRYING = false

    shroom_counter = 1
    shroom_timer = 120
    shroom_order = { 1, 2, 0, 2, 0, 2 }
    shroom_spot = { move_spots[1], move_spots[2], move_spots[3], move_spots[9], move_spots[10] }

    -- make_bottle(3, 0)
    -- make_bottle(-3, 0)
    make_shroom(-8, 0)
    -- make_bottle(-3, 0, true)

end

function item_loop()

    if key("z") then
        if zready then
            zready = false
            local ilist = items:list()
            local gottem = false
            -- local caul_stacked = 0
            for i = 1, #ilist do
                local item = ilist[i]
                if not item.carried then
                    if player.x > item.ent.x - 1 and player.x < item.ent.x + 1 then
                        -- if player.z + 2 > item.ent.z - 2 and player.z - 1 < item.ent.z + 2 then
                        item.carried = true
                        carry:add(item)
                        gottem = true
                        break;
                        -- elseif abs(item.ent.x) < 3 then
                        -- mix_in(item)
                        -- end
                    end
                end
            end
            local carried = carry:list()
            -- print("p" .. gottem)
            -- if not picked any more up, drop all our carried
            if not gottem then
                for i = #carried, 1, -1 do
                    local c = carried[i]
                    c.carried = false
                    if abs(player.x) < 7 then
                        c.ent.z = 2 + (i - 1) + #MIXER
                        c.ent.x = 0
                        mix_in(c)
                    else

                        c.ent.z = player.z + (i - 1)

                        -- print("stink" .. c.stink)

                        if c.stinky ~= nil then
                            if c.stinky then
                                anti_guy(move_pos.x)
                            else
                                anti_zom(move_pos.x)
                            end
                            print "droppy"
                            kill(c.ent)
                            items:remove(c)
                        end
                    end
                    carry:remove(c)
                    print(i)
                end
            end
        end

    else
        zready = true
    end

    local carried = carry:list()
    for i = 1, #carried do
        move(carried[i].ent, player, i)
    end

    CARRYING = #carried > 0

    shroom_timer = shroom_timer - 1
    if shroom_timer <= 0 then
        shroom_timer = 120
        local x = shroom_spot[flr(rnd() * #shroom_spot)]
        local ilist = items:list()
        local can_plant = true
        for i = 1, #ilist do
            local item = ilist[i]
            if not item.carried and (item.ent.x > x - 2 and item.ent.x < x + 2) then
                can_plant = false
                break
            end
        end
        if can_plant then
            make_shroom(x, 0)
        end
    end

    if x_timer > 0 then
        x_timer = x_timer - 1
        if x_timer == 0 and x_dialog then
            kill(x_dialog)
            x_dialog = nil
            -- print("kill dialog")
        end
    end
    -- local ilist = items:list()
    -- for i=1,#ilist do
    --     local item =ilist[i]
    --     if not item.carried then

    --     end
    -- end
end

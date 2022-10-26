function make_bottle(x, z)
    local b = spawn("potions0", x, MID, z - 0.5)
    local bb = { ent = b, vz = 0, carried = false, active = false }
    items:add(bb)
end

function make_shroom(x, z)
    local type = flr(rnd() * 3)
    local b = spawn("mushroom" .. type, x, MID, z - 0.5)
    local bb = { ent = b, vz = 0, carried = false, active = false, type = type }
    items:add(bb)
end

local zready = true

function init_items()
    items = Dict:new()
    carry = Dict:new()

    make_bottle(3, 0)
    make_bottle(-3, 0)
    make_shroom(-8, 0)
end

function item_loop()

    if key("z") then
        if zready then
            zready = false
            local ilist = items:list()
            local gottem = false
            for i = 1, #ilist do
                local item = ilist[i]
                if not item.carried then
                    if player.x + 1 > item.ent.x - 2 and player.x - 1 < item.ent.x + 2 then
                        if player.z + 2 > item.ent.z - 2 and player.z - 1 < item.ent.z + 2 then
                            item.carried = true
                            carry:add(item)
                            gottem = true
                            break;
                        end
                    end
                end
            end
            local carried = carry:list()
            -- print("p" .. gottem)
            if not gottem then
                -- print(#carried)
                for i = #carried, 1, -1 do
                    local c = carried[i]
                    c.carried = false
                    c.ent.z = player.z + (i - 1)
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
    -- local ilist = items:list()
    -- for i=1,#ilist do
    --     local item =ilist[i]
    --     if not item.carried then

    --     end
    -- end
end

function init_directions()
    -- spawn("mushroom0", -3, MID, 4)
    -- spawn("mushroom1", -2, MID, 4)
    -- spawn("mushroom2", -1, MID, 4)
    -- spawn("equals", 0, MID, 4)
    -- spawn("mushroom2", 1, MID, 4)

    -- spawn("banner0", -3, MID + 0.5, 4)
    -- spawn("banner1", -2, MID + 0.5, 4)
    -- spawn("banner1", -1, MID + 0.5, 4)
    -- spawn("banner1", 0, MID + 0.5, 4)
    -- spawn("banner2", 1, MID + 0.5, 4)

    banner({ "mushroom0", "mushroom0", "mushroom2", "equals", "potions1" }, -3, MID, 4)
    banner({ "mushroom1", "mushroom2", "mushroom2", "equals", "potions0" }, -3, MID, 5.5)


end

function banner(t, x, y, z)

    for i, value in ipairs(t) do
        spawn(value, x + i, y, z)
        local s = 1
        if i == 1 then
            s = 0
        elseif i == #t then
            s = 2
        end
        spawn("banner" .. s, x + i, y + 0.5, z)
    end
end

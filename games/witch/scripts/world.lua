local function wood()
    local t = rnd()
    -- .25 .75 .375
    if t > .55 then
        return "wood3"
    elseif t > .10 then
        return "wood4"
    else
        return "wood5"
    end
    -- return "wood" .. (flr(rnd() * 3) + 3)
end

cube("portrait", "wood3", "wood7")
cube("roof", "wood3", "wood3", "wood3", "wood3", "wood3", "wood5")

function wallB(x)
    for i = 1, 12 do
        for j = 0, 6 do
            tile(wood(), x + i - 6, 18, j)
        end
    end

    tile("wood8", x - 3, 18, 1)
    tile("wood8", x - 3, 18, 4)
    tile("wood8", x + 3, 18, 1)
    tile("wood8", x + 3, 18, 4)

    tile("portrait", x - 1, 18, 1, 1)
end

function wallF(x)
    for i = 1, 8 do
        for j = 2, 6 do
            tile(wood(), x, 10 + i, j)
        end
    end
    for i = 3, 8 do
        for j = 0, 2 do
            tile(wood(), x, 10 + i, j)
        end
    end
    tile("wood-door.doorframe", x, 12, 1)
    tile("wood-door.doorframe", x, 11, 1)


    tile("portrait", x, 16, 1, tern(x > 0, 0, 2))
    tile("wood8", x, 14, 1)
    tile("wood8", x, 15, 1)
    -- tile("wood7", x, 15, 1)
end

function floor(x)
    for i = -6, 6 do
        for j = 1, 8 do
            tile(wood(), x + i, j + 10, -1)
        end
    end
end

function roadup(x)
    for i = 4, 16 do
        tile("witch-grass1", x - 5 + i, MID + 1, -1, 2)
        tile("witch-grass" .. (flr(rnd() * 2) + 7), x - 5 + i, MID, -1, flr(rnd() * 4))
        tile("witch-grass1", x - 5 + i, MID - 1, -1, 4)
    end
    tile("witch-grass4", x + 11, MID - 1, -1, 1)
    tile("witch-grass1", x + 12, MID + 1, -1, 2)
end

function water(x)
    for i = -2, 6 do
        for j = 0, 6 do
            tile("witch2", x + 4 + j, MID + i, -1, 0)
        end
    end

end

function roadin(x)
    for i = 1, 12 do
        tile("witch-grass1", x - 1, i, -1, 3)
        tile("witch-grass" .. (flr(rnd() * 2) + 7), x, i, -1, flr(rnd() * 4))
        tile("witch-grass1", x + 1, i, -1, 1)
    end
end

function roof(x)
    for i = -6, 6 do
        for j = 1, 8 do
            tile("roof", x + i, j + 10, 7)
        end
    end
end

function treeline(x, s, e)
    for i = -5, 5 do
        for j = s, e do
            if j % 4 == 0 and abs(i + j) % 3 == 0 then
                tile("witch-blocks", x + i + flr(rnd() * 3), -12 + j, 0, flr(rnd() * 4))
            end
        end
    end
    for i = -5, 5 do
        for j = 1, 90 do
            v = 0
            t = 0
            -- local l = rnd()
            -- if l > 0.8 then
            --     v = 3
            -- elseif l > 0.7 then
            --     v = tern(rnd() > 0.5, 7, 8)
            --     t = rnd() * 4
            --     -- tile("witch-grass6", x + i - 1, -12 + j - 1, -1, 0)
            --     -- tile("witch-grass1", x + i - 1, -12 + j, -1, 0)
            --     -- tile("witch-grass6", x + i + 1, -12 + j - 1, -1, 0)
            --     -- tile("witch-grass1", x + i, -12 + j - 1, -1, 0)

            --     -- tile("witch-grass6", x + i + 1, -12 + j + 1, -1, 0)
            --     -- tile("witch-grass1", x + i + 1, -12 + j, -1, 0)
            --     -- tile("witch-grass6", x + i - 1, -12 + j + 1, -1, 0)
            --     -- tile("witch-grass1", x + i, -12 + j + 1, -1, 0)
            -- end

            v = tern(rnd() > 0.5, 0, 3)
            tile("witch-grass" .. (v), x + i, -12 + j, -1, t)
        end
    end
end

function light_tree(x)

    tile("witch-blocks", x - 4, 4, 0, flr(rnd() * 4.))
    tile("witch-blocks", x + 3, 4, 0, flr(rnd() * 4.))
    tile("witch-blocks", x + 4, 4, 0, flr(rnd() * 4.))

end

function just_grass(x)
    for i = -6, 6 do
        for j = 1, 90 do
            tile("witch-grass" .. tern(rnd() > 0.5, 0, 3), x + i, -12 + j, -1, 0)
        end
    end
end

function graves(x)

    for i = -3, 5, 2 do
        if i <= 3 then
            tile("tombstone", x + i - 1, 13, 0, 0)
            tile("witch-grass" .. tern(rnd() > .5, 7, 8), x + i - 1, 12, -1)
        end
        tile("tombstone", x + i - 1, 6, 0, 0)
        tile("witch-grass" .. tern(rnd() > .5, 7, 8), x + i - 1, 5, -1, flr(rnd() * 4.))

    end
end

-- function bld()
--     for i = -200, 200 do
--         -- for j = 0, 10 do
--         local j = tiler
--         -- if (i + j) % 2 == 0 then
--         -- local l = "witch" .. math.floor(rnd() * 3)
--         local l = "xub"
--         -- log(l)
--         local h = 1
--         if (i + j) % 10 == 0 then
--             h = cos(i / 10.) * 8.
--         end
--         tile(l, i, j + 10, h - 12, flr(rnd() * 6))
--         -- end
--         -- end
--     end
-- end


-- cool shape
-- for i = -8, 8 do
--     for j = -8, 8 do
--         for k = -8, 8 do
--             if abs(i) + abs(j) + abs(k) == 16 then
--                 tile("witch2", i, j + 12, k)
--             end
--         end
--     end
-- end

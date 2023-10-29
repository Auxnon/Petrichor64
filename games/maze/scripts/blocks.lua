-- numbers = {}
function make_blocks()
    mod("wall",
        {
            t = { "blocks7", "blocks11", "blocks11", "blocks11", "blocks11" },
            q = { { 0, .5, 1 }, { 1, .5, 1 }, { 1, 1, 1 }, { 0, 1, 1 },
                { 0, .5, 1 }, { 1, .5, 1 }, { 1, .5, 0 }, { 0, .5, 0 },
                { 0, 1,  1 }, { 1, 1, 1 }, { 1, 1, 0 }, { 0, 1, 0 },
                { 0, .5, 1 }, { 0, 1, 1 }, { 0, 1, 0 }, { 0, .5, 0 },
                { 1, .5, 1 }, { 1, 1, 1 }, { 1, 1, 0 }, { 1, .5, 0 }

            }
        })

    mod("wallbend",
        {
            t = { "blocks10", "blocks11", "blocks11", "blocks11", "blocks11", "blocks11", "blocks11" },
            q = { { 1, 0, 1 }, { 1, 1, 1 }, { 0, 1, 1 }, { 0, 0, 1 },
                { 0, .5, 1 }, { .5, .5, 1 }, { .5, .5, 0 }, { 0, .5, 0 },
                { .5, .5, 1 }, { .5, 0, 1 }, { .5, 0, 0 }, { .5, .5, 0 },
                { 1,  1,  1 }, { 1, 0, 1 }, { 1, 0, 0 }, { 1, 1, 0 },
                { 1, 1, 1 }, { 0, 1, 1 }, { 0, 1, 0 }, { 1, 1, 0 },
                { 0, 1, 1 }, { 0, .5, 1 }, { 0, .5, 0 }, { 0, 1, 0 },
                { .5, 0, 1 }, { 1, 0, 1 }, { 1, 0, 0 }, { .5, 0, 0 },
            }
        })

    for i = 0, 9 do
        local n = nimg(32, 32)
        n:text("" .. i, 12, 10)
        tex("t" .. i, n)
        mod("" .. i, { t = { "t" .. i }, q = { { -.5, -.5, 0 }, { .5, -.5, 0 }, { .5, .5, 0 }, { -.5, .5, 0 } } })
        -- numbers["" .. i] = n
    end
end

--- @param label string
function block(i, j, k, label)
    tile("blocks8", i, j + 1, k)
    tile("blocks8", i - 1, j, k, 1)
    tile("blocks8", i, j - 1, k, 2)
    tile("blocks8", i + 1, j, k, 3)

    tile("blocks12", i - 1, j + 1, k)
    tile("blocks12", i - 1, j - 1, k, 1)
    tile("blocks12", i + 1, j - 1, k, 2)
    tile("blocks12", i + 1, j + 1, k, 3)

    tile("blocks13", i, j, k)

    if label:len() > 1 then
        -- text(label, i, j, k + 1)
        if label:len() > 2 then
            if label == "blue" then
                tile("blocks2", i, j, k)
                guy1 = make('guys0', i, j, k + 1)
                add_item(guy1)
            else
                tile("blocks3", i, j, k)
                guy2 = make('guys1', i, j, k + 1)
                add_item(guy2)
            end
        else
            -- print(label:sub(1, 1) .. " and ", label:sub(2))
            local t = make(label:sub(1, 1), i - .2, j, k + .6)
            t.scale = 2
            local t = make(label:sub(2), i + .2, j, k + .6)
            t.scale = 2
        end
    else
        -- print(label:sub(1, 1) .. " only")
        local t = make(label:sub(1, 1), i, j, k + .6)
        t.scale = 2
    end


    -- tile("wall", i, j + 1, k + 1)
    -- tile("wall", i - 1, j, k + 1, 1)
    -- tile("wall", i, j - 1, k + 1, 2)
    -- tile("wall", i + 1, j, k + 1, 3)
    -- tile("wallbend", i + 1, j + 1, k + 1)
    -- tile("wallbend", i - 1, j + 1, k + 1, 1)
    -- tile("wallbend", i - 1, j - 1, k + 1, 2)
    -- tile("wallbend", i + 1, j - 1, k + 1, 3)
end

function wall(i, j, k, xd, yd)
    if xd ~= nil then
        if xd >= 0 then
            tile("wall", i + 1, j, k + 1, 3)
            if yd == nil then
                tile("wall", i + 1, j + 1, k + 1, 3)
                tile("wall", i + 1, j - 1, k + 1, 3)
            end
        end
        if xd <= 0 then
            tile("wall", i - 1, j, k + 1, 1)
            if yd == nil then
                tile("wall", i - 1, j + 1, k + 1, 1)
                tile("wall", i - 1, j - 1, k + 1, 1)
            end
        end
    end

    if yd ~= nil then
        if yd >= 0 then
            tile("wall", i, j + 1, k + 1, 0)
            if xd == nil then
                tile("wall", i + 1, j + 1, k + 1, 0)
                tile("wall", i - 1, j + 1, k + 1, 0)
            end
        end
        if yd <= 0 then
            tile("wall", i, j - 1, k + 1, 2)
            if xd == nil then
                tile("wall", i + 1, j - 1, k + 1, 2)
                tile("wall", i - 1, j - 1, k + 1, 2)
            end
        end
    end
end

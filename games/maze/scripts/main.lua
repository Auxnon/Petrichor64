sky()
fill("FF0")

attr { modernize = false, glitch = { 0.1, 0.5, 0 }, curvature = 0.97, resolution = 480 }

board_size = 2
function tern(a, b, c)
    if a then
        return b
    else
        return c
    end
end

function board()
    -- for i = -board_size, board_size do
    --     for j = -board_size, board_size do
    --         local v = irnd(3)
    --         tile(tern(v == 2, 0, "blocks" .. v), i, j, 0)
    --     end
    -- end
    -- tile("blocks2", -board_size - 1, 0, 0)
    -- tile("blocks3", board_size + 1, 0, 0)
    local b = { { 8, 6, 4, 2, "blue" },
        { 10, 12, 14, 16, 18 },
        { 19, 21, 23, 22, 20 },
        { 17, 15, 13, 11, 9 },
        { "red", 1, 3, 5, 7 } }

    local walls = { { { nil, nil }, { nil, -1 }, { nil, -1 }, { nil, -1 }, { nil, -1 } },
        { { nil, -1 }, { nil, -1 }, { nil, -1 }, { nil, -1 }, { nil, nil } },

        { { nil, nil }, { nil, -1 }, { nil, -1 }, { nil, -1 }, { nil, -1 } },
        { { nil, -1 }, { nil, -1 }, { nil, -1 }, { nil, -1 }, { nil, nil } },
        { { nil, nil }, { nil, nil }, { nil, nil }, { nil, nil }, { nil, nil } } }

    for i = -2, 2 do
        for j = -2, 2 do
            local ni = i + 3
            local nj = 6 - (j + 3)
            local n = "" .. b[nj][ni]
            -- local n = ((i + 3) + (j + 2) * 5)
            block(i * 3, j * 3, 0, n)
            local w = walls[nj][ni]
            wall(i * 3, j * 3, 0, w[1], w[2])
        end
    end

end

function pieces()
    die = spawn("die", 0, 0, 1)
    die.offset = { -0.5, -0.5, -0.5 }
    add_item(die)
    add_item(guy1)
    add_item(guy2)
end

function controls()

    if key("w") then
        pos.y = pos.y + sin(rot) * speed
        pos.x = pos.x + cos(rot) * speed
    elseif key("s") then
        pos.y = pos.y - sin(rot) * speed
        pos.x = pos.x - cos(rot) * speed
    end
    if key("a") then
        pos.y = pos.y + cos(rot) * speed
        pos.x = pos.x - sin(rot) * speed
    elseif key("d") then
        pos.y = pos.y - cos(rot) * speed
        pos.x = pos.x + sin(rot) * speed
    end
    local f = -8
    if key("q", true) then
        local x = pos.x - cos(rot) * f
        local y = pos.y - sin(rot) * f
        rot = rot + tau / 8
        pos.x = x + cos(rot) * f
        pos.y = y + sin(rot) * f
    elseif key("e", true) then
        local x = pos.x - cos(rot) * f
        local y = pos.y - sin(rot) * f
        rot = rot - tau / 8
        pos.x = x + cos(rot) * f
        pos.y = y + sin(rot) * f
    end

    local m = mouse()
    local v = 10
    local f = -(pos.z) / (m.vz)
    local p = { x = pos.x + f * m.vx, y = pos.y + f * m.vy, z = 0 + pos.z + f * m.vz }
    cursor.x = p.x
    cursor.y = p.y
    cursor.z = p.z
    shadow.x = p.x
    shadow.y = p.y

    if m.m1 then
        if mouse_reset then
            mouse_once = true
            mouse_reset = false
        else
            mouse_once = false
        end
    else
        mouse_once = false
        mouse_reset = true
        item_release_check()
    end
end

speed = 0.1
pos = { x = 0, y = -8, z = 8 }
rot = pi / 2
mouse_reset = true
mouse_once = false

function main()
    make_blocks()
    model("cursor", { t = { "blocks9" }, q = { { 0, 1, 1 }, { 1, 1, 1 }, { 1, 0, 1 }, { 0, 0, 1 } } })
    model("shadow", { t = { "shadow" }, q = { { 0, 1, 1 }, { 1, 1, 1 }, { 1, 0, 1 }, { 0, 0, 1 } } })
    cursor = spawn("cursor", 0, 0, 0)
    cursor.offset = { -0.5, -.5, -0.4 }
    shadow = spawn("shadow", 0, 0, 0)
    shadow.offset = { -0.5, -.5, -0.45 }
    model("die", { t = { "dice0", "dice1", "dice2", "dice3", "dice4", "dice5" } })
    pieces()
    board()
end

function loop()
    controls()
    check_items()
    -- die.rz = die.rz + 0.06
    cam { pos = { pos.x, pos.y, pos.z }, rot = { rot, -tau / 8 } }
end

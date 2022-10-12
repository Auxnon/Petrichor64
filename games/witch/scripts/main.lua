sky()
fill("544e68")
img("twilight")
text("MakeAvoy", 0, 180)
line(.85, .85, 1, .85)
line(.85, .85, .85, 1)
line(.85, .85, 1, 1)
gui()
cube("xub", "witch0", "witch1", "witch0", "witch1", "witch2", "witch2")
crt({
    -- dark = 0.
    -- low = 0.9,
    -- high = .6
    modernize = 1
})

function tern(cond, T, F)
    if cond then
        return T
    else
        return F
    end
end

key_delay = 0

function wallB(x)
    for i = 1, 12 do
        for j = 0, 6 do
            tile("witch0", x + i - 6, 18, j)
        end
    end
end

function wallF(x)
    for i = 1, 8 do
        for j = 2, 6 do
            tile("witch0", x, 10 + i, j)
        end
    end
    for i = 3, 8 do
        for j = 0, 2 do
            tile("witch0", x, 10 + i, j)
        end
    end
end

function floor(x)
    for i = -6, 6 do
        for j = 1, 8 do
            tile("witch0", x + i, j + 10, -1)
        end
    end
end

function treeline(x)
    for i = -5, 5, 2 do
        for j = 1, 90 do
            if (i * j) % 5 == 0 then
                tile("witch-blocks", x + i, -12 + j, 0, flr(rnd() * 4))
            end

        end
    end
    for i = -5, 5 do
        for j = 1, 90 do
            v = 0
            t = 0
            local l = rnd()
            if l > 0.8 then
                v = 3
            elseif l > 0.7 then
                v = tern(rnd() > 0.5, 7, 8)
                t = rnd() * 4
                tile("witch-grass6", x + i - 1, -12 + j - 1, -1, 0)
                tile("witch-grass1", x + i - 1, -12 + j, -1, 0)
                tile("witch-grass6", x + i + 1, -12 + j - 1, -1, 0)
                tile("witch-grass1", x + i, -12 + j - 1, -1, 0)

                tile("witch-grass6", x + i + 1, -12 + j + 1, -1, 0)
                tile("witch-grass1", x + i + 1, -12 + j, -1, 0)
                tile("witch-grass6", x + i - 1, -12 + j + 1, -1, 0)
                tile("witch-grass1", x + i, -12 + j + 1, -1, 0)
            end
            tile("witch-grass" .. (v), x + i, -12 + j, -1, t)
        end
    end
end

local l = {
    [1] = {
        [1] = 0,
        [2] = 0
    }
}
function main()

    player = spawn('witchy', 0, 12, -.5)
    DISTANCE = 12
    room_pos = {
        x = 0,
        y = 0
    }
    move_pos = {
        x = 0,
        y = 0
    }
    move_index = 2
    move_spots = {-8, -3, 3, 8}
    cam_pos = {
        x = 0,
        y = -4,
        z = 2
    }
    tiler = 0

    tile("witch0", 0, 12, -1)
    wallB(0)
    wallF(-6)
    wallF(6)
    floor(0)
    tile("witch1", -DISTANCE, 12, -1)
    tile("witch2", DISTANCE, 12, -1)
    treeline(DISTANCE)
    treeline(-DISTANCE)

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

    tile("witch0", 0, 12, 0)
    enemy_start()
    particle_init()

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

function clamp()
    if room_pos.x > DISTANCE then
        room_pos.x = DISTANCE
    elseif room_pos.x < -DISTANCE then
        room_pos.x = -DISTANCE
    end
end

local r = 0
local tile_delay = 0
function loop()
    -- r = r + 0.001
    -- camrot(r, 0)
    -- clear_tiles()
    -- tiler = tiler + 1
    -- bld()

    tile_delay = tile_delay + 1
    -- enemy_loop()
    particle_loop()
    if key_delay > 10 then
        if key("a") then

            room_pos.x = room_pos.x - DISTANCE
            if move_index == 4 then
                move_index = 3
            else
                move_index = 1
            end
            move_pos.x = move_spots[move_index]
            clamp()
            key_delay = 0

        elseif key("d") then
            room_pos.x = room_pos.x + DISTANCE
            if move_index == 1 then
                move_index = 2
            else
                move_index = 4
            end
            move_pos.x = move_spots[move_index]
            clamp()
            key_delay = 0
        end
    else
        key_delay = key_delay + 1
    end

    player.x = player.x + (move_pos.x - player.x) / 6.
    cam_pos.x = cam_pos.x + (room_pos.x - cam_pos.x) / 4.
    -- sky()

    clr()
    m = mouse()

    for i = 2, #l do
        line(l[i - 1][1], l[i - 1][2], l[i][1], l[i][2])
    end

    l[#l + 1] = {
        [1] = m[1],
        [2] = m[2]
    }

    if #l > 30 then
        table.remove(l, 1)
    end

    sqr(100, 100, 10, 10)

    -- img("zom", 255, 1)
    -- line(0, 0, 1, 1)
    cam_pos.y = cam_pos.y + 0.005
    text(cam_pos.y, 0, 200)
    campos(cam_pos.x, cam_pos.y, cam_pos.z)

    -- camrot(1.57, r)
    -- r = r - 0.001
    -- cam_pos.z = cam_pos.z + 0.002
    -- log("x" .. player.y)
    -- player.y = example.y + rnd() * 0.1 - 0.05
end

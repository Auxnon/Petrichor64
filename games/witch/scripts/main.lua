sky()
fill("544e68")
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
        for j = 1, 30 do
            if (i * j) % 5 == 0 then
                tile("witch-blocks", x + i, 12 + j, 0, flr(rnd() * 4))
            end
        end
    end
end

function main()

    player = spawn('witchy', 0, 12, -0.5)
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
        y = 0
    }

    log('main runs once everything has loaded')
    tile("witch0", 0, 12, -1)
    wallB(0)
    wallF(-6)
    wallF(6)
    floor(0)
    tile("witch1", -DISTANCE, 12, -1)
    tile("witch2", DISTANCE, 12, -1)
    treeline(DISTANCE)
    treeline(-DISTANCE)
    enemy_start()

end

function clamp()
    if room_pos.x > DISTANCE then
        room_pos.x = DISTANCE
    elseif room_pos.x < -DISTANCE then
        room_pos.x = -DISTANCE
    end
end

-- local r = 0
function loop()
    -- r = r + 0.001
    -- camrot(r, 0)
    enemy_loop()
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
    clr()
    text(cam_pos.x)
    campos(cam_pos.x, cam_pos.y, 2)
    -- log("x" .. player.y)
    -- player.y = example.y + rnd() * 0.1 - 0.05
end

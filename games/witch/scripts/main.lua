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

MID = 12



function tern(cond, T, F)
    if cond then
        return T
    else
        return F
    end
end

key_delay = 0


local l = {
    [1] = {
        [1] = 0,
        [2] = 0
    }
}
function main()




    DISTANCE = 12
    room_pos = {
        x = 0,
        y = 0
    }

    move_index = 5
    move_spots = { -27, -20, -17, -8, -3, 3, 8, 17, 20, 27 }
    cam_pos = {
        x = 0,
        y = -4,
        z = 2
    }

    move_pos = {
        x = move_spots[move_index],
        y = 0
    }

    player = spawn('wood-door.doorframe', move_spots[move_index], MID, -.5)
    -- player:anim("attack")

    tiler = 0

    just_grass(0)
    treeline(0, 40, 90)
    tile("witch0", 0, MID, -1)
    wallB(0)
    wallF(-6)
    rdoor = spawn("wood-door.door", 6, 12.5, -0.5)
    ldoor = spawn("wood-door.door", -6, 12.5, -0.5)
    cauldron = spawn("cauldron", 0, MID, 0.5)
    wallF(6)
    floor(0)
    roof(0)
    tile("witch1", -DISTANCE, MID, -1)
    tile("witch2", DISTANCE, MID, -1)

    treeline(DISTANCE * 2, 30, 90)
    light_tree(2 * DISTANCE)
    roadin(DISTANCE * 2)




    treeline(DISTANCE, 30, 90)
    -- graves(DISTANCE)
    light_tree(DISTANCE)
    -- light_tree(DISTANCE)
    roadup(DISTANCE)

    treeline(-DISTANCE * 2, 30, 90)
    light_tree(-2 * DISTANCE)

    treeline(-DISTANCE, 30, 90)
    graves(-DISTANCE)


    treeline(3 * DISTANCE, 1, 90)
    treeline(-3 * DISTANCE, 1, 90)

    water(DISTANCE * 2)

    --

    -- tile("witch0", 0, MID, 0)
    enemy_start()
    Particle_Init()
    init_items()

    -- local im = gimg("example")
    -- for i = 1, #im.data do
    --     log("#" .. im.data[i])
    -- end
    -- log("image is " .. im.w .. "," .. im.h .. " and " .. im.data)
end

local function clamp(before)

    if move_index == 1 or move_index == 2 then
        room_pos.x = -2 * DISTANCE
    elseif move_index == 3 or move_index == 4 then
        room_pos.x = -DISTANCE
    elseif move_index == 5 or move_index == 6 then
        room_pos.x = 0
    elseif move_index == 7 or move_index == 8 then
        room_pos.x = DISTANCE
    elseif move_index == 9 or move_index == 10 then
        room_pos.x = 2 * DISTANCE
    end

    -- door check
    if (before == 4 and move_index == 5) then
        ldoor_dest = 2 * 3.14
        door_delay = 60
    end
    if (before == 5 and move_index == 4) then
        ldoor_dest = 3.14
        door_delay = 60

    end

    if (before == 6 and move_index == 7) then
        rdoor_dest = 2 * 3.14
        door_delay = 60
    end
    if (before == 7 and move_index == 6) then
        rdoor_dest = 3.14
        door_delay = 60
    end

end

local tile_delay = 0
door_delay = 60
ldoor_dest = 3 * 3.14 / 2
rdoor_dest = 3 * 3.14 / 2
function loop()
    -- door.rot_z = -3.14 / 2

    tile_delay = tile_delay + 1
    enemy_loop()
    Particle_Loop()
    item_loop()
    if key_delay > 10 then
        if key("a") then

            -- room_pos.x = room_pos.x - DISTANCE
            -- if move_index == 4 then
            --     move_index = 3
            -- else
            --     move_index = 1
            -- end
            local before = move_index
            if move_index > 1 then
                move_index = move_index - 1
            end
            move_pos.x = move_spots[move_index]


            -- room_pos.x = (move_index / 2) * DISTANCE
            clamp(before)
            key_delay = 0

        elseif key("d") then
            -- room_pos.x = room_pos.x + DISTANCE
            -- if move_index == 1 then
            --     move_index = 2
            -- else
            --     move_index = 4
            -- end

            local before = move_index
            if move_index < #move_spots then
                move_index = move_index + 1
            end
            move_pos.x = move_spots[move_index]
            -- room_pos.x = ((move_index-#move_spots) / 2) * DISTANCE

            clamp(before)
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

    if door_delay > 0 then
        -- print("door" .. ldoor.rot_z)
        if door_delay < 30 then
            ldoor_dest = 3 * 3.14 / 2
            rdoor_dest = 3 * 3.14 / 2
        end
        ldoor.rot_z = ldoor.rot_z - (ldoor.rot_z - ldoor_dest) / 10.
        rdoor.rot_z = rdoor.rot_z - (rdoor.rot_z - rdoor_dest) / 10.
        door_delay = door_delay - 1
    end



    sqr(0.2, 0.8, 0.2, 0.1)

    text(player.x, 0, 200)
    campos(cam_pos.x, cam_pos.y, cam_pos.z)



    -- camrot(1.57, r)
    -- r = r - 0.001
    -- cam_pos.z = cam_pos.z + 0.002
    -- log("x" .. player.y)
    -- player.y = example.y + rnd() * 0.1 - 0.05
end

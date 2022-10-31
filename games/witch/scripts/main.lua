sky()
fill("544e68")
img("twilight")
text("MakeAvoy", 0, 180)
-- line(.85, .85, 1, .85)
-- line(.85, .85, .85, 1)
-- line(.85, .85, 1, 1)
gui()
cube("xub", "witch0", "witch1", "witch0", "witch1", "witch2", "witch2")
crt({
    -- dark = 0.
    -- low = 0.9,
    -- high = .6
    modernize = 1
})

MID = 12

LOST = false



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

function first_load()
    TITLE_UP = true
    DISTANCE = 12
    cam_pos = {
        x = 0,
        y = 0,
        z = room_pos.z or 2
    }

    room_pos = {
        x = 0,
        y = -4,
        z = 2
    }

    move_index = 5
    move_spots = { -27, -20, -17, -8, -3, 3, 8, 17, 20, 27 }


    move_pos = {
        x = move_spots[move_index],
        y = 0
    }

    player = spawn('witchy3', move_spots[move_index], MID, -.5)
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
    reset()
    enemy_start()
    Particle_Init()
    init_items()
    init_directions()
    init_stinks()
    survive_timer = 0
    second_timer = 0
end

function main()
    room_pos = {
        x = 0,
        y = 0,
        z = 14
    }
    logo()

    -- local im = gimg("example")
    -- for i = 1, #im.data do
    --     log("#" .. im.data[i])
    -- end
    -- log("image is " .. im.w .. "," .. im.h .. " and " .. im.data)
end

function reset()
    LDOOR_HP = 100
    POT_HP = 100
    LDOOR_COMPARE = 100
    POT_COMPARE = 100
    LEFT_DOOR = true
    ldoor.z = -0.5
    ldoor.rot_x = 0
    cauldron.rot_y = 0
    current_text_timer = 0
    hint_text "Move with A,D"
end

function hint_text(t)
    current_text_timer = 480
    current_text = t
end

function door_check()

    if LEFT_DOOR then
        local fl = flr(LDOOR_HP)
        if fl ~= LDOOR_COMPARE then
            LDOOR_COMPARE = fl
            ldoor.rot_x = (rnd() - 0.5) / 2.

            if fl <= 0 then
                ldoor.z = -10
                LEFT_DOOR = false
            end

        end
    else
        local fl = flr(POT_HP)
        if fl ~= POT_COMPARE then
            POT_COMPARE = fl
            cauldron.rot_y = (rnd() - 0.5) / 3.

            if fl <= 0 then
                lose()
            end
        end
    end
end

function lose()
    print "UH OH"
    LOST = true
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

door_delay = 60
ldoor_dest = 3 * 3.14 / 2
rdoor_dest = 3 * 3.14 / 2
local text_timer = 0
function loop()
    -- door.rot_z = -3.14 / 2
    if player then
        enemy_loop()
        Particle_Loop()
        item_loop()
        door_check()
        stink_loop()
        difficulty_loop()

        if LOST then
            sqr(60, 100, 200, 16)
            text("Ya got unwitched :\\ ", 60, 100)
            player:tex("witchy1")

            sqr(60, 190, 200, 16)
            text("Survived  " .. survive_timer .. " seconds", 60, 190)

            if key("z") or key("x") or key("c") or key("space") or key("a") or key("d") then
                reload()
            end
        else
            player_move_loop()
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





            local txt = ""
            if current_text then
                txt = current_text
                current_text_timer = current_text_timer - 1
                if current_text_timer <= 0 then
                    current_text = nil
                end
            else
                txt = "Time survived " .. survive_timer
            end
            local ii = 0
            for w in string.gmatch(txt, ".") do
                -- local tt = txt
                -- local t = txt:sub(i, 1)

                ii = ii + 1
                local xx = text_timer + ii
                text(w, 20 + (ii * 6), 200 + flr(sin(xx / 6.) * 6))
                -- text(w, 20 + (ii * 6), 200 + flr(sin(xx / 6.) * 6))
                -- print(t)
            end
            text_timer = text_timer + 1

            if TITLE_UP then
                img("title", 96, 128)
            end

            campos(cam_pos.x, cam_pos.y, cam_pos.z)

            second_timer = second_timer + 1
            if second_timer >= 60 then
                second_timer = 0
                survive_timer = survive_timer + 1
            end

        end

        -- camrot(1.57, r)
        -- r = r - 0.001
        -- cam_pos.z = cam_pos.z + 0.002
        -- log("x" .. player.y)
        -- player.y = example.y + rnd() * 0.1 - 0.05


    else
        if logo_delay > 0 then
            logo_delay = logo_delay - 1
            logo_loop()
        else
            first_load()
        end
    end
end

function first_check()
    if TITLE_UP then
        TITLE_UP = false
        hint_text "Pick up mushrooms with Z, X, C. Spell space."
    end
end

function player_move_loop()
    if key_delay > 10 then
        if key("a") then
            first_check()
            local before = move_index
            if move_index > 1 then
                move_index = move_index - 1
            end
            move_pos.x = move_spots[move_index]


            clamp(before)
            key_delay = 0

        elseif key("d") then
            first_check()
            local before = move_index
            if move_index < #move_spots then
                move_index = move_index + 1
            end
            move_pos.x = move_spots[move_index]

            clamp(before)
            key_delay = 0
        end
    else
        key_delay = key_delay + 1
    end
    local player_diff = (move_pos.x - player.x) / 6.
    player.x = player.x + player_diff
    cam_pos.x = cam_pos.x + (room_pos.x - cam_pos.x) / 4.
    cam_pos.y = cam_pos.y + (room_pos.y - cam_pos.y) / 20.

    local logo_dist = (cam_pos.z - room_pos.z)
    if logo_dist > 0 then
        cam_pos.z = cam_pos.z - 0.1
    end

    if player_diff > 0 then
        player.flipped = false;
    else
        player.flipped = true;
    end
    if abs(player_diff) > 0.01 then
        player:tex("witchy2")
    else
        if CARRYING then
            player:tex("witchy6")
        else
            player:tex("witchy3")
        end

    end

end

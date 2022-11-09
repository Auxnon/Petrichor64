cube("logodirt", "logo12", "logo18", "logo18", "logo18", "logo18", "logo18")
logo_delay = 160
local dirt
local cloud
local dirt_z = 0
pi = 3.1457
tau = pi * 2
TITLE_UP = false
function logo()
    local top = room_pos.z
    campos(0, 0, top)
    for i = -8, 8 do
        for j = -5, 5 do
            tile("logo19", i, 10, j + top)
        end
    end

    -- local back = spawn("example", 0, 1, 2)
    dirt = spawn("logodirt", -0, -2, -1.5 + top)
    dirt.rot_z = -tau + tau / 8
    dirt.rot_x = 0

    -- dirt.rot_z = 0



    cloud = spawn("pcloud", -0.25, 4, 1.5 + top)
    cloud.rot_z = tau / 16
    cloud.rot_x = tau / 16



end

function logo_loop()
    if key("z") or key("x") or key("c") or key("space") then
        logo_delay = 0
        -- reload()
    end

    local top = room_pos.z
    if dirt.rot_z < (tau / 8) then
        dirt.rot_z = dirt.rot_z + 00.08
        if dirt.rot_z >= (tau / 8) then
            for i = -0.375, 0.375, 0.1 do
                local rain = spawn("logo14", i, 6, -0.5 + top + cos(i * 20) * 0.4 + 0.5)
            end

            for i = 0, 5 do
                local lo = spawn("logo" .. i, i + -1.75, 6, -2.0 + top)
            end
            text("Interpreted Game System", 40, 220)
        end

    end

    if dirt.rot_x < (tau / 16) then
        dirt.rot_x = dirt.rot_x + 00.002
    end
    if dirt.y < 8 then
        dirt.y = dirt.y + 0.1
    end
    if cloud.z > (0.75 + top) then
        cloud.z = cloud.z - 0.01
    end

    -- sqr(0, 0, 200, 200)
    -- dir  t.rot_z = dirt.rot_z + (tau / 8) / 20.
end

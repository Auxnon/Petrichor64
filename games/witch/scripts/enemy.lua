local zoms = {}
function zombie()
    local z = spawn("zom", -DISTANCE + (rnd() * 8) - 4, 12, -0.5)
    zoms[#zoms + 1] = z
    -- z:anim("rise")
end
local delay = 0
function enemy_loop()
    if delay > 100 then
        zombie()
        delay = 0
    end
    for i = 1, #zoms do
        local z = zoms[i]
        z.x = z.x + 0.01
    end
    delay = delay + 1
end

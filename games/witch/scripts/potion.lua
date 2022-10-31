MIXER = {}
x_timer = 0
x_dialog = nil
FIRST_POTION = true

function mix_in(p)
    MIXER[#MIXER + 1] = p
    -- print("mixin" .. #MIXER)
    if #MIXER >= 3 then
        local stinks = 0
        local darks = 0
        local brights = 0
        for i = 1, 3 do
            if MIXER[i].type == 1 then
                brights = brights + 1
            elseif MIXER[i].type == 0 then
                stinks = stinks + 1
            elseif MIXER[i].type == 2 then
                darks = darks + 1
            end
            items:remove(MIXER[i])
            if MIXER[i].ent then
                kill(MIXER[i].ent)
            end
        end
        if brights == 1 and darks == 2 then
            first_potion_check()
            make_bottle(tern(rnd() > 0.5, 3, -3), 0, false)
            make_check()
        elseif stinks == 2 and darks == 1 then
            first_potion_check()
            make_bottle(tern(rnd() > 0.5, 3, -3), 0, true)
            make_check()
        else
            make_x()
        end
    end
end

function first_potion_check()
    if FIRST_POTION then
        FIRST_POTION = false
        hint_text "Drop specific potion outside to ward off"
    end
end

function make_x()
    -- spawn
    x_dialog = spawn("x", 0, MID - 2, 0)
    x_timer = 120
end

function make_check()
    -- spawn
    x_dialog = spawn("check", 0, MID - 2, 0)
    x_timer = 120
end

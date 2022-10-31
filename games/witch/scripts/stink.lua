ZOM_BAN = false
GUY_STINK = false
ZOM_HOOK = -1
GUY_HOOK = -1
stinkers = {}

function init_stinks()
    stinkers = Dict:new()
end

function anti_guy(x)
    print "anti guy"
    make_smell(x, true)
end

function anti_zom(x)
    print "anti zom"
    make_smell(x, false)
end

function make_smell(x, stinky)
    -- 1,2
    -- 3,4
    -- mid 5,6
    -- 7,8
    -- 9,10
    -- move_spots={ -27, -20, -17, -8, -3, 3, 8, 17, 20, 27 }

    local start = 0
    local stop = 0
    local room = 0
    for i = 1, #move_spots, 2 do
        if x > (move_spots[i] - 1) then
            room = (i + 1) / 2
            start = move_spots[i]
            stop = move_spots[i + 1]
            -- break;
        end
    end


    print("room" .. room .. " start " .. start .. " and stop " .. stop)

    local scents = {}
    local smell_type = 0
    if stinky then
        smell_type = 2
    end
    local range = stop - start
    for i = 1, 10 do
        scents[#scents + 1] = spawn("smell" .. smell_type, start + rnd() * range, MID - 2, 1 + rnd() * 4)
    end

    local smeller = { room = room, scents = scents, delay = 60 * 10 }

    stinkers:add(smeller)

    if room == 2 and not stinky then
        ZOM_BAN = true
        ZOM_HOOK = smeller.id
        print("zom hooked to " .. smeller.id)
    elseif room == 4 and stinky then
        GUY_STINK = true
        GUY_HOOK = smeller.id
    end

    return { start, stop }
end

function stink_loop()
    local slist = stinkers:list()
    for i = #slist, 1, -1 do
        local s = slist[i]
        if s.delay > 0 then
            s.delay = s.delay - 1
        else
            for j = 1, 10 do
                kill(s.scents[j])
            end

            if ZOM_BAN and ZOM_HOOK == s.id then
                ZOM_BAN = false
            elseif GUY_STINK and GUY_HOOK == s.id then
                GUY_STINK = false
            end

            stinkers:remove(s)
        end

    end
end

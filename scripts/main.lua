

local a=ent("chicken",0,0)
a.scale(2)
a.brain("walker")

rot=0.

function loop()
    cam({0,0,0},{20*math.cos(rot),20*math.sin(rot),16.})
    rot=rot+0.1
    if rot>math.pi*2 then
        rot=0
    end
end


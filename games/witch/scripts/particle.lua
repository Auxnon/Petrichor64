particles = {}
function particle_init()
    for i = 1, 100 do
        local p = spawn("leaf", (rnd() * 20) - 10, 12, 4, 50)
        local leaf = {
            ent = p,
            vx = rnd() * 0.05,
            vy = 0,
            vz = -rnd() * 0.05
        }
        particles[#particles + 1] = leaf
    end
end

function particle_loop()
    for i = 1, #particles do
        local l = particles[i]
        l.ent.x = l.ent.x + l.vx
        l.ent.z = l.ent.z + l.vz
        if l.ent.z < 0 then
            l.ent.z = 4
            l.ent.x = rnd() * 40 - 20
            l.ent.y = rnd() * 20
        end
    end
end

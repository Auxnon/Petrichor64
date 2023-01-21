smodel("card-model",
    { t = "test-card", v = { { -0.8, -1, 0 }, { 0.8, -1, 0 }, { 0.8, 1, 0 }, { -0.8, 1, 0 } }, i = { 0, 1, 2, 0, 2, 3 },
        u = { { 0, 0 }, { 1, 0 }, { 1, 1 }, { 0, 1 } } })

sky()
fill(1, 1, .4, 1)
gui()
cards = {}
function ran(a)
    return (rnd() - 0.5) * a * 2
end

function main()
    log('main runs once everything has loaded')
    for i = 1, 1000 do
        local t = spawn('card-model', 0, i / 10., -2)
        t.vy = 0.1
        -- t.vx = 0.03
        -- t.vz = 0.04
        table.insert(cards, t)
    end
end

function loop()
    local c = #cards
    for i = 1, c do
        local t = cards[i]
        t.y = t.y + t.vy
        t.x = cos(t.y / 4) * 6
        t.z = -2 + sin(t.y / 7) * -6
        t.rot_y = t.rot_y + 0.01
        t.rot_x = t.rot_x + 0.01
        t.rot_z = t.rot_z + 0.01
        if t.y > c / 10. then
            t.y = 1. / 10.
            t.x = 0
            t.z = -2
            -- t.rot_x = 0
            -- t.rot_y = 0
            -- t.rot_z = 0
        end

    end
end

mod("card-model",
    {
        t = { "card" },
        v = { { -0.8, -1, 0 }, { 0.8, -1, 0 }, { 0.8, 1, 0 }, { -0.8, 1, 0 } },
        i = { 0, 1, 2, 0, 2, 3 },
        u = { { 0, 0 }, { 1, 0 }, { 1, 1 }, { 0, 1 } }
    })

sky:fill { 1, 1, .4, 1 }
--- @type entity[]
cards = {}
function ran(a)
    return (rnd() - 0.5) * a * 2
end

function main()
    cout('main runs once everything has loaded')
    for i = 1, 1000 do
        local t = make('card-model', ran(10), rnd() * 8 + 4, ran(10))
        t.rz = rnd() * tau
        t.ry = rnd() * tau
        t.rx = rnd() * tau
        t.vx = rnd() / 10.
        t.vy = rnd() / 10.
        t.vz = -rnd() / 3.
        table.insert(cards, t)
    end
    -- example = spawn('card-model', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
end

function loop()
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
    for i = 1, #cards do
        local t = cards[i]
        t.x = t.x + t.vx
        t.y = t.y + t.vy
        t.z = t.z + t.vz
        t.z = t.z + 0.01
        t.y = t.ry + 0.01
        t.rx = t.rx + 0.01
        if t.x > 10 then t.vx = -t.vx end
        if t.x < -10 then t.vx = -t.vx end
        if t.y > 10 then t.vy = -t.vy end
        if t.y < -10 then t.vy = -t.vy end
        if t.z < -10 then
            t.vz = -rnd() / 3.
            t.z = 10
        end
    end
end

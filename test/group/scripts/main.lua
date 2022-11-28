cube("grib", "grid")
bg(1, 1, .4, 1)
ex = {}
example = spawn('grib', 0, 6, 0)
local last = example
for i = 1, 10, 1 do
    local e = spawn('grib', 0, 2, 0)
    group(last, e)
    last = e
    ex[i] = e
end
local bot = spawn("bot.bot", 0, 6, -1)
group(last, bot)
last = bot

for i = 11, 20, 1 do
    local e = spawn('grib', 0, 2, 0)
    group(last, e)
    last = e
    ex[i] = e
end

function main()
    log('main runs once everything has loaded')
    -- group(last, bot)
end

function loop()
    local mice = mouse()
    local m = { x = mice[1] * 2 - 1, y = -mice[2] * 2 + 1 }
    example.x = m.x
    example.z = m.y
    -- bot.x = m.x
    -- bot.z = m.y * 2 - 1
    -- for i = 1, #ex do
    --     ex[i].rot_z = mice[1] * 6.28
    --     ex[i].rot_x = mice[2] * 6.28
    -- end

    -- example.x = example.x + math.random() * 0.1 - 0.05
    -- example.y = example.y + math.random() * 0.1 - 0.05
end

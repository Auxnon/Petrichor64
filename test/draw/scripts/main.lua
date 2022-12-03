-- example = spawn('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
math.randomseed(os.time())
bg(1, 1, .4, 1)
function main()
    log('main runs once everything has loaded')
    for key, value in pairs(_G) do
        print(key)
    end
end

function rando()
    return math.random()
    -- return rnd()
end

function loop()
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05

    -- for i = 0, 12, 1 do
    --     for j = 0, 12, 1 do
    --         -- print("." .. (i * j))
    --         pixel(i, j, rando(), rando(), rando(), 1.)
    --     end
    -- end
    -- print "success"
end

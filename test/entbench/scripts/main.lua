sky:fill("f88")
function bunch(i)
    for j = 1, 100 do
        make('square', (rnd() - .5) * 20, i, (rnd() - .5) * 20)
    end
    -- spawn('example', rnd() * 3. - 1.5,12, rnd() * 3. - 1.5)
end

function main()
    cout('main runs once everything has loaded')
end

player = { x = 0, y = 0, z = 0 }
n = 0
function loop()
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
    if n < 200 then
        n = n + 1
        bunch(n + 30)
    end
    if key("w") then
        player.y = player.y + 0.1
    elseif key("s") then
        player.y = player.y - 0.1
    end

    if key("a") then
        player.x = player.x - 0.1
    elseif key("d") then
        player.x = player.x + 0.1
    end
    cam { pos = { player.x, player.y, player.z } }
    local t = cin()
    if #t > 0 then
        print(t)
    end
end

-- while moving W 40-43fps on macos external 120hz 2k monitor

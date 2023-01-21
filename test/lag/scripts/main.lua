example = spawn('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
sky()
loop_count = 0
local clock = os.clock
function sleep(n) -- seconds
    local t0 = clock()
    while clock() - t0 <= n do end
end

function main()
    log('lag for ~6 seconds every loop')
    return 1
end

function loop()
    sleep(6)
    example.x = example.x + rnd() * 0.1 - 0.05
    example.z = example.z + rnd() * 0.1 - 0.05
    loop_count = loop_count + 1
    if loop_count % 2 == 0 then
        fill("855")
    else
        fill("585")
    end
    print("looped " .. loop_count .. " times")
end

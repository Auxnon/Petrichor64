bg(1, 1, .4, 1)
function main()
    example = spawn('officechair.chair', 0, 2, 0)
    log('main runs once everything has loaded')
    list = lmodel('')
    for l = 1, #list do
        print(list[l])


    end

    for i = -10, 10, 2 do for j = -10, 10, 2 do tile("chair", i, j, -1) end end
end

function loop()
    -- example.x = example.x + rnd() * 0.01 - 0.005
    -- example.z = example.z + rnd() * 0.01 - 0.005
    example.y = example.y + 0.01
end

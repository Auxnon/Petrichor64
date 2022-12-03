example = spawn('example', rando() * 3. - 1.5, 12, rando() * 3. - 1.5)
bg(1, 1, .4, 1)
function main()
    log('main runs once everything has loaded')
    local test = 1
    if type(test) == "number" then
        log('true is a boolean')
    else
        log('true is not a boolean')

    end
    print("ugh" .. type(test))
end

function loop()
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
end

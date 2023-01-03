example = spawn('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
sky()
fill(1, 1, .4, 1)
gui()
function main()
    log('main runs once everything has loaded')
end

function loop()
    example.x = example.x + rnd() * 0.1 - 0.05
    example.z = example.z + rnd() * 0.1 - 0.05

    dimg()
end

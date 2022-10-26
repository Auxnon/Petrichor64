example = spawn('beveled_cube.2', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
camrot(1.57, 0.4)
bg(1, 1, .4, 1)
function main()
    log('main runs once everything has loaded')
end
function loop()
    example.x = example.x + rnd() * 0.1 - 0.05
    example.z = example.z + rnd() * 0.1 - 0.05
end

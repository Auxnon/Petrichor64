example = spawn('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
sky()
fill(0, 0, .2)
gui()
function main()
    log('main runs once everything has loaded')
    im = gimg("example")
end

function loop()
    example.x = example.x + rnd() * 0.1 - 0.05
    example.z = example.z + rnd() * 0.1 - 0.05
    im:clr()
    im:line(0, 0, rnd(), rnd())
    simg("example", im)
    clr()
    dimg(im, 0, 0)
end

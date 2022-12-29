ex = {}
for i = 1, 10000 do
    ex[i] = spawn('example', (rnd() - 0.5) * 20, 12 + rnd() * 300, (rnd() - 0.5) * 20)
end

sky()
fill(0, 0, .2)
gui()
function main()
    log('main runs once everything has loaded')
    im = gimg("example")
    song { 440., 261.63, 440., 261.63 }
end

function loop()
    for i = 1, #ex do
        ex[i].x = ex[i].x + rnd() * 0.1 - 0.05
        ex[i].z = ex[i].z + rnd() * 0.1 - 0.05
    end
    -- im:clr()
    im:rect(rnd(), rnd(), 4, 4, rnd(), rnd(), rnd())
    simg("example", im)
    clr()
    dimg(im, 0, 0)
    -- print("" .. example)

end

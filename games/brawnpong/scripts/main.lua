sky()
fill(1, 1, .4, 1)
gui()
function main()
    log('main runs once everything has loaded')
    pong_init()
    example = spawn('pong', 0, 2, 0)
end

function loop()
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
    pong_loop()
    -- dimg(pong_screen)
end

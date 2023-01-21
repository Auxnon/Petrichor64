sky()
fill(1, 1, .4, 1)
gui()
function main()
    -- cube("test", "flat0")
    example = spawn('cube', 0, 12, -4)
    example:stex("flat1")
    example.scale = 2
    example.rot_x = tau / 8
    log('main runs once everything has loaded')
end

function loop()
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
end

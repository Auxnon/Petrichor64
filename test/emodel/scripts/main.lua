-- example = spawn('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
sky()
fill(1, 1, .4, 1)
gui()

-- campos(0, -12, 3)
camr = { x = 0, y = 0 }
cam = { x = 0, y = 0, z = 0 }

points = {}

function make_cube()
    es(spawn('example', -1, -1, 0))
    es(spawn('example', 1, -1, 0))
    es(spawn('example', -1, 1, 0))
    es(spawn('example', 1, 1, 0))
    es(spawn('example', -1, -1, 2))
    es(spawn('example', 1, -1, 2))
    es(spawn('example', -1, 1, 2))
    es(spawn('example', 1, 1, 2))

    spawn('example', 1, 1, 4)
    spawn('example', 1, 1, 5)
    ee = spawn('example', 1, -1, 4)
    ee.scale = 1
    ee.rot_x = rnd()
    ee.rot_y = rnd()
    ee.rot_z = rnd()
end

function es(a)
    a.scale = 1
    points[#points + 1] = a
end

function main()
    make_cube()
    camcheck()
    local g = gimg "example"
    g:line(0, 0, 16, 16, "f00")
    simg("example", g)
    log('main runs once everything has loaded')
end

function camcheck()
    cam.x = -12 * cos(camr.x)
    cam.y = -12 * sin(camr.x)
    cam.z = 3
    camrot(camr.x, camr.y)
    campos(cam.x, cam.y, cam.z)
end

function loop()


    if key("a", true) then
        camr.x = camr.x + tau / 8
        camcheck()
    elseif key("d", true) then
        camr.x = camr.x - tau / 8
        camcheck()
    end
    ee.rot_z = ee.rot_z + .1

end

example = spawn('example', 0, 0, 5)
bg(1, 1, .4, 1)

level = {{1, 0, 1, 1}, {1, 1, 0, 1}, {1, 0, 0, 1}, {1, 0, 0, 1}}
camera = {
    x = -335,
    y = 0,
    z = 0,
    r = 3.14
}

function main()
    log('main runs once everything has loaded')
    -- f = io.open("games/midnight/assets/test.txt", "rb")
    -- txt = f:read("*all")
    -- log("loaded txt")
    -- log(txt)

    -- for ii = 1, #level do
    --     i = (ii - 1) % 5
    --     j = flr((ii - 1) / 5)
    --     if level[ii] == 1 then
    --         -- text(i .. "," .. j, i * 24, j * 6)
    --         tile("beveled_cube.block", 0, i, j, rnd() * 4)

    --     end
    -- end

    makey()
    -- for i = -8, 8 do
    --     for j = -8, 8 do
    --         tile("beveled_cube.block", 0, i, j, rnd() * 4)
    --     end
    -- end

end

function makey()
    clear_tiles()
    for i = 1, #level do
        for j = 1, #level[i] do
            if level[i][j] == 1 then
                text(i .. "," .. j, i * 24, j * 6)
                tile("beveled_cube.block", 0, j, 5 - i, rnd() * 4)
            end
        end
    end
end

function loop()
    -- camera.x = camera.x - 0.2
    camera.r = camera.r + 0.002
    campos(cos(camera.r) * camera.x, sin(camera.r) * camera.x, camera.z)
    camrot(camera.r, 0, 0)

    x = example.y
    y = example.z

    if key("a") then
        x = x - 0.1
    end
    if key("d") then
        x = x + 0.1
    end
    -- clr()
    -- text("test", camera.r * 10, 10)
    example.y = x
    example.z = y
end

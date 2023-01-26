sky()
fill("FF0")

function tern(a, b, c)
    if a then
        return b
    else
        return c
    end
end

function board()
    local size = 2
    for i = -size, size do
        for j = -size, size do
            local v = irnd(3)
            tile(tern(v == 2, 0, "blocks" .. v), i, j, 0)
        end
    end
    tile("blocks2", -size - 1, 0, 0)
    tile("blocks3", size + 1, 0, 0)
    guy1 = spawn('guys0', -size - 1, 0, 1)
    guy2 = spawn('guys1', size + 1, 0, 1)
end

function main()
    cube("die", "dice0", "dice1", "dice2", "dice3", "dice4", "dice5")
    for i = -3, 3 do
        tile("die", i, 4, 1, irnd(6))
    end
    board()
    cam { pos = { 0, -8, 8 }, rot = { pi / 2, -tau / 8 } }
end

function loop()
end

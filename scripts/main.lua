math.randomseed(os.time())
local fellas = {}

-- top west north east south bottom
cube("block1", "ground9", "ground7", "ground7", "ground7", "ground7", "ground15")
cube("block2", "ground10", "ground7", "ground7", "ground7", "ground7", "ground15")
cube("block3", "ground17", "ground7", "ground7", "ground7", "ground7", "ground15")
cube("block4", "ground18", "ground7", "ground7", "ground7", "ground7", "ground15")
cube("block5", "ground36", "ground7", "ground7", "ground7", "ground7", "ground15")
cube("dirt", "ground15", "ground15", "ground15", "ground15", "ground15", "ground15")

cube("east", "ground9", "ground7", "ground7", "compass2", "ground7", "ground14")
cube("west", "ground9", "compass0", "ground7", "ground7", "ground7", "ground14")
cube("north", "ground9", "ground7", "compass1", "ground7", "ground7", "ground14")
cube("south", "ground9", "ground7", "ground7", "ground7", "compass3", "ground14")

e2 = {
    frame = 0,
    delay = 0
}

player = spawn("dog5", 0, 0, 0)

function copy(o)
    local c
    if type(o) == 'table' then
        c = {}
        for k, v in pairs(o) do
            c[k] = copy(v)
        end
    else
        c = o
    end
    return c
end

function main()
    log("heya")
    bg(0, .6, .9, 1)

    size = 20
    half = 0
    t = 0
    for i = -size, size do
        for j = -size, size do
            mx = 4 + math.sqrt(i * i + j * j) / 20.
            t = t + 1
            r = math.floor(math.random() * 4) + 1
            h = 0
            block = ""
            if math.floor(math.random() * 8) == 0 then
                h = math.floor(math.random() * 3) + 1
                block = "block5"
            else
                block = "block" .. r
            end
            if math.abs(i) > 4 or math.abs(j) > 4 then
                tile(block, (i - half), (j - half), h + mx - 12)
                for k = 0, h do
                    tile("dirt", (i - half), (j - half), k + mx - 13)
                end
            else
                tile("grass0", (i - half), (j - half), -8)
            end
        end
    end
    tile("east", -4, 0, -2)
    tile("west", 4, 0, -2)
    tile("north", 0, 4, -2)
    tile("south", 0, -4, -2)
    tile_done()

    h = 8
    c = 0

    for i = -h, h do
        for j = -h, h do
            c = c + 1
            t = spawn(c % 2 == 0 and "dog0" or "chicken", i, j, -7)
            e = {
                frame = 0,
                delay = 0,
                data = t
            }
            fellas[#fellas + 1] = e
            -- fellas[#fellas + 1] = t

        end
    end
end

dir = 0.01
incr = -60
delay = 0
vel = 0
function loop()
    for i = 1, #fellas do
        walker(fellas[i])
    end

    if key("W") then
        player.y = player.y + 0.1
    end
    if key("S") then
        player.y = player.y - 0.1
    end

    if key("A") then
        player.x = player.x - .1
    end

    if key("D") then
        player.x = player.x + .1
    end
    if key("G") then
        vel = 0.1
    end

    if not is_tile(player.x, player.y, player.z - 2) then
        vel = vel - 0.001
        player.z = player.z + vel
    end

end


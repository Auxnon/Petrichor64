math.randomseed(os.time())
local fellas = {}
crt({
    resolution = 480,
    curvature = 0.9,
    flatness = 3.,
    glitch = 6.0,
    dark = .8,
    low = .3,
    high = .8,
    bleed = .3
})
sky()
fill(0., .6, .9)

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
anim("dog idle", { "dog0", "dog1", "dog2" }, 2)

e2 = {
    frame = 0,
    delay = 0
}

player = spawn("midnightman0", 0, 0, 0)

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

function tiler()
    -- clear_tiles()
    size = 20
    half = 0
    for i = -size, size do
        for j = -size, size do
            mx = 4 + math.sqrt(i * i + j * j) / 20.
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
                tile("grass0", (i - half), (j - half), h + mx - 12)
                for k = 0, h do
                    tile("grass0", (i - half), (j - half), k + mx - 13)
                end
            else
                tile("grid", (i - half), (j - half), -8)
            end
        end
    end

    tile("east", -4, 0, -2)
    tile("west", 4, 0, -2)
    tile("north", 0, 4, -2)
    tile("south", 0, -4, -2)

end

function main()
    -- log("heya")


    tiler()

end

dir = 0.01
incr = -60
delay = 0
vel = 0
camx = 0
camy = 0
posx = 0.
posy = 0.
function loop()
    for i = 1, #fellas do
        walker(fellas[i])
    end

    if button("dleft") then
        camx = camx - 0.4
    end
    if button("dright") then
        camx = camx + 0.4
    end

    if button("dup") then
        camy = camy + 0.4
    end
    if button("ddown") then
        camy = camy - 0.4
    end

    camx = camx - analog("raxisx") / 100.
    camy = camy + analog("raxisy") / 100.

    posx = posx + analog("laxisx")
    posy = posy + analog("laxisy")

    local moving = false
    if key("W") then
        player.y = player.y + 0.01
        camy = camy + 0.01
        moving = true
    end
    if key("S") then
        player.y = player.y - 0.01
        camy = camy - 0.01
        moving = true
    end

    if key("A") then
        player.x = player.x - .1
        camx = camx + 0.01
        moving = true
    end

    if key("D") then
        player.x = player.x + .1
        camx = camx - 0.01
        moving = true
    end

    if key("left") then
        posx = posx + 1.
    end

    if key("right") then
        posx = posx - 1.
    end

    if key("up") then
        posy = posy + 1.
    end

    if key("down") then
        posy = posy - 1.
    end

    camrot(camx, camy, 0)
    campos(posx, posy, 0)
    if moving then
        player:anim("Walk")
    else
        player:anim("Idle")
    end

    if key("space") then
        vel = 0.1
    end

    if not is_tile(player.x, player.y + 0.5, player.z - 0.5) then
        vel = vel - 0.005
        player.z = player.z + vel

    else
        player.z = player.z + math.abs(vel) + 0.001
        vel = -vel * .5

    end

end

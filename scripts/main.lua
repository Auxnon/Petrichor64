math.randomseed(os.time())
local fellas = {}

-- top west north east south bottom
_cube("block1", "ground9", "ground7", "ground7", "ground7", "ground7", "ground15")
_cube("block2", "ground10", "ground7", "ground7", "ground7", "ground7", "ground15")
_cube("block3", "ground17", "ground7", "ground7", "ground7", "ground7", "ground15")
_cube("block4", "ground18", "ground7", "ground7", "ground7", "ground7", "ground15")
_cube("block5", "ground36", "ground7", "ground7", "ground7", "ground7", "ground15")
_cube("dirt", "ground15", "ground15", "ground15", "ground15", "ground15", "ground15")

_cube("east", "ground9", "ground7", "ground7", "compass2", "ground7", "ground14")
_cube("west", "ground9", "compass0", "ground7", "ground7", "ground7", "ground14")
_cube("north", "ground9", "ground7", "compass1", "ground7", "ground7", "ground14")
_cube("south", "ground9", "ground7", "ground7", "ground7", "compass3", "ground14")
-- _cube("test", "grass0", "ground7", "ground7", "ground7", "compass3", "ground14")

function _main()
    _bg(0, .6, .9, 1)
    size = 20
    half = 0
    t = 0
    for i = -size, size do
        for j = -size, size do
            mx = 4 + math.sqrt(i * i + j * j) / 20.
            -- h = (math.sin(i / 10.) + math.cos(j / 10.)) * mx + mx
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
                _tile(block, (i - half), (j - half), h + mx - 12)
                for k = 0, h do
                    _tile("dirt", (i - half), (j - half), k + mx - 13)
                end
            else
                _tile("grass0", (i - half), (j - half), -8)
            end
        end
    end
    _tile("east", -4, 0, 0)
    _tile("west", 4, 0, 0)
    _tile("north", 0, 4, 0)
    _tile("south", 0, -4, 0)
    _tile_done()

    h = 8
    c = 0
    for i = -h, h do
        for j = -h, h do
            c = c + 1
            t = _spawn(c % 2 == 0 and "dog1" or "chicken", i, j, -7)
            fellas[#fellas + 1] = t

        end
    end
    -- _print("count is"..c)
end

bg = 1
dir = 0.01
incr = -60
delay = 0
function _loop()
    for i = 1, #fellas do
        _walker(fellas[i])
    end
    -- bg = bg - dir
    -- if bg > 1 then
    --     bg = 1
    --     dir = -dir
    -- elseif bg < 0 then
    --     bg = 0
    --     dir = -dir
    -- end
    -- _bg(1 - bg, bg, bg, 1)

    if _space() then
        delay = delay + 1
        if delay > 10 then
            delay = 0
            for i = -50, 50 do
                _tile("grass12", i, incr, -12)
                _tile("0", i, incr - 10, -12)
            end
            _tile_done()
            incr = incr + 1
        end
    end
end

-- function loop()
--     cam({0,0,0},{20*math.cos(rot),20*math.sin(rot),16.})
--     rot=rot+0.1
--     if rot>math.pi*2 then
--         rot=0
--     end
-- end

-- _bg(0,1,1,1)


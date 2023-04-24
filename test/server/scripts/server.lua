-- Codex 3.0.0 "Artichoke"
local delay = 0
function main()
    -- spawn("")
    -- local status, err = xpcall(function()
    --     servy = conn("localhost:3000", false, true)
    --     cout 'server started'
    -- end, function(err)
    --     cout("error" .. err)
    -- end)
    servy = conn("192.168.1.42:3000", false, true)
    -- cout("servy")
    -- print("" .. err)
    cout("servy2")
end

function loop()
    -- cout("1looping ")
    local r = servy:recv()
    if r then
        cout("got" .. r)
        servy:send(r)
    end
    -- delay = delay + 1
    -- if delay > 60 then
    --     -- cout("looping " .. rnd())
    --     delay = 0
    -- end
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
end

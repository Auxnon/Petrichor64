-- Codex 2.0.0 "Avocado"
sky()
fill('FF5')
gui()
-- typeface={}
function main()
    local m = loadfile("../hullo.lua")()
    print(" m is " .. m)

    local t = nimg(2047, 12)
    -- load


    local alphanum = "abcdefghijklmnopqrstuvwxyz0123456789{}\":"
    for i = 1, alphanum:len() do
        local c = alphanum:sub(i, i)
        local im = nimg(10, 10)
        im:text(c)
        tex(c, im)
        -- typeface[c]=i
    end
    local s = [[{"error":{"message":"self is not defined","name":"ReferenceError","stack":"ReferenceError: self is not defined\n    at Object.95380 (/home/makeavoy/cr-preview/packages/core-client/dist/sw.js:2:712554)\n    at he (/home/makeavoy/cr-preview/packages/core-client/dist/sw.js:2:749890)\n    at /home/makeavoy/cr-preview/packages/core-client/dist/sw.js:2:750615\n    at /home/makeavoy/cr-preview/packages/core-client/dist/sw.js:2:791311\n    at Object.<anonymous> (/home/makeavoy/cr-preview/packages/core-client/dist/sw.js:2:791315)\n    at Module._compile (node:internal/modules/cjs/loader:1126:14)\n    at Object.Module._extensions..js (node:internal/modules/cjs/loader:1180:10)\n    at Module.load (node:internal/modules/cjs/loader:1004:32)\n    at Function.Module._load (node:internal/modules/cjs/loader:839:12)\n    at Module.require (node:internal/modules/cjs/loader:1028:19)"},"level":"error","message":"Unhandled rejection","pid":2674900,"promise":{},"timestamp":"2023-02-03T19:11:26.339Z","version":"2.59.1"}]]
    -- local s = "This is an especially long line of text so we can hopefully get an idea about how to expect this to be which is very long"

    -- print("s length " .. s:len())
    -- t:text(s)
    -- tex("test", t)
    -- local w = s:len()
    -- model("long", {
    --     t = { "test" },
    --     v={ { 0, 0, 1 }, { w, 0, 1 }, { w, 0, 0 }, { 0, 0, 0 }  },
    --     u={ { 0, 0 }, { 1, 0 }, { 1, 1 }, { 0, 1 }},
    -- i={  0, 1, 2  , 0, 2, 3 }
    -- })

    -- example = spawn('long', rnd() * 3. - 1.5, 24, rnd() * 3. - 1.5)
    -- example.scale=0.5
    -- example= spawn('example',-1,12,0)
    for i = 1, s:len() do
        local c = s:sub(i, i)
        local ii = i % 32
        local jj = flr(i / 32)
        local e = spawn(c, 0.75 * ii - 12, 48, 18 - jj)
        -- group(example,e)
        -- typeface[c]=i
    end
end

function loop()
    -- example.x -=0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
    -- example.rz+=tau/1000.
    -- spawn('long', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
end

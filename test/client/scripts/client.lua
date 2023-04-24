-- Codex 3.0.0 "Artichoke"
example = make('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
function main()
    servy = conn("192.168.1.42:3000")
    cout 'client running'
end

function loop()
    if key("space", true) then
        servy:send("hi")
        cout("sent hi")
    end
    local r = servy:recv()
    if r then
        cout("got", r)
        -- servy:send(r)
    end
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
end

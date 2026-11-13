-- The host channel, and what the host's policy left this cart (web/two.html).
-- Sends "conn=... io=..." once, "echo <msg>" for each host message, and
-- "key k" when k is pressed while it has the keyboard.
-- silt-safe style (AGENTS.md): no calls inside argument lists or concatenations.

function has(x)
    if x then
        return 'yes'
    end
    return 'nil'
end

function main()
    S = { said = false, last = 'nothing yet', t = 0 }
    sky:fill('036')
    -- on a <petrichor-64> element: its `title` event, not document.title
    attr({ title = 'host-echo' })
end

function loop()
    S.t = S.t + 1
    local c = has(conn)
    local i = has(io)
    if host then
        if S.said == false then
            S.said = true
            local report = 'conn=' .. c .. ' io=' .. i
            host.send(report)
        end
        -- keys only reach the focused engine: say when this one hears one
        local k = key('k', true)
        if k then
            host.send('key k')
        end
        local m = host.recv()
        while m do
            S.last = m
            local reply = 'echo ' .. m
            host.send(reply)
            m = host.recv()
        end
    end
    local l1 = 'conn ' .. c
    local l2 = 'io ' .. i
    local l3 = 'got ' .. S.last
    local x = S.t % 120
    gui:clr()
    gui:text('host-echo', 8, 8, 'FFF')
    gui:text(l1, 8, 24, 'FF0')
    gui:text(l2, 8, 40, 'FF0')
    gui:text(l3, 8, 56, '0F0')
    gui:rect(8 + x, 80, 16, 16, 'F80')
end

function draw(w, h)
end

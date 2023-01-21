pong1 = { y = 0, dir = 3. }
pong2 = { y = 0, dir = 0 }
ball = { x = 0, y = 0, vx = 0, vy = 0 }

function pong_init()
    pong_screen = nimg(256, 256)
    simg("pong", pong_screen)
end

function pong_move(p, dir)
    if p == 1 then
        pong1.dir = dir
    else
        pong2.dir = dir
    end
end

function clamp(v, min, max)
    if v < min then
        return min
    elseif v > max then
        return max
    else
        return v
    end
end

function pong_loop()
    pong1.y = pong1.y + pong1.dir
    pong2.y = pong2.y + pong2.dir
    -- pong1.y = clamp(pong1.y, 0, 100)
    if pong1.y < 0 then
        pong1.dir = abs(pong1.dir)
        pong1.y = 0
    elseif pong1.y > 256 then
        pong1.dir = -abs(pong1.dir)
        pong1.y = 256
    end
    pong2.y = clamp(pong2.y, 0, 100)
    pong_screen:fill("0f0")
    pong_screen:line(10, flr(pong1.y), 10, flr(pong1.y + 20), "f00")
    pong_screen:line(246, flr(pong2.y), 246, flr(pong2.y + 20), "00f")
    local pw = 256 - pong1.y / 2
    local ph = 256 - pong1.y / 4
    pong_screen:rrect(flr(pong1.y / 2), flr(pong1.y / 4), pw, ph, .2, "f00")
    simg("pong", pong_screen)

end

pong1 = { y = 0, dir = 0 }
pong2 = { y = 0, dir = 0 }
ball = { x = 0, y = 0, vx = 0, vy = 0 }
pong_screen = nimg(256, 256)
function pong_init()

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
    pong1.y = clamp(pong1.y, 0, 100)
    pong2.y = clamp(pong2.y, 0, 100)

end

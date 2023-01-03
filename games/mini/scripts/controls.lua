camspeed = 0.15
function loop_controls()
    if key("w") then
        camera.y = camera.y + camspeed
    elseif key("s") then
        camera.y = camera.y - camspeed
    end

    if key("a") then
        camera.x = camera.x - camspeed
    elseif key("d") then
        camera.x = camera.x + camspeed
    end

end

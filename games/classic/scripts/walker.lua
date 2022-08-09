function walker(entity)
    if key("A") then
        entity.data:anim("Walk")
        entity.data.z = -7 + math.cos(os.clock() * 2. + entity.data.x + entity.data.y) * 0.6
    else
        entity.data:anim("Idle")
    end

    if key("D") then
        entity.delay = entity.delay + 1
        if entity.delay > 10 then
            entity.delay = 0
            entity.frame = entity.frame + 1
            if entity.frame > 2 then
                entity.frame = 0
            end
            -- entity.data:tex("dog" .. entity.frame)
        end
    elseif key("C") then
        -- entity.data:tex("chicken")
    end

    return entity
end


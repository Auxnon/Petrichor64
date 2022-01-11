
function biggo(entity)
    -- entity.x=16+math.cos(os.clock()*1.)*16.
    -- entity.y=16+math.sin(os.clock()*2.)*16.
    spawn("dude",entity.x,entity.y)

    return entity --make_ent(p.x+1.,p:get_y())
end

return biggo


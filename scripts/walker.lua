
function big_ol_function(entity)
    entity.x=entity.x+math.cos(os.clock()*4.)*0.1
    entity.y=entity.y+math.sin(os.clock()*4.)*0.1
    --entity.x=entity.x+0.2
    return entity --make_ent(p.x+1.,p:get_y())
end

return big_ol_function


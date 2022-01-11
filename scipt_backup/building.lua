
function big_ol_function(entity)
    entity.x=math.cos(os.clock()*4.)*2.
    entity.z=math.sin(os.clock()*4.)*2.
    entity.rot_y=entity.rot_y+0.1;
    return entity --make_ent(p.x+1.,p:get_y())
end

return big_ol_function


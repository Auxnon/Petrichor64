function _walker(entity)
    entity.z = -7 + math.cos(os.clock() * 4. + entity.x + entity.y) * 0.6
    -- entity.y=entity.y+math.sin(os.clock()*4.)*0.1
    -- entity.x=entity.x+0.2
    return entity -- make_ent(p.x+1.,p:get_y())
end

function testum(e)
    return "hello"
end

-- return big_ol_function


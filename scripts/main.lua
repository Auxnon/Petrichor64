

-- local a=ent("chicken",0,0)
-- a.scale(2)
-- a.brain("walker")

-- rot=0.
-- _spawn(0,0,0)

function _main()
    _ents={[0]={x=43}}
    -- _testo={53}
    _print('hi there')
    _bg(0,1,1,1)
    -- for i=0,10 do
    --     _spawn(i+5,0,2)
    -- end

    _print(69)

    h=10
    c=0
    for i=-h,h do
        for j=-h,h do
            for k=-h,h do
                t=_spawn(i,j,k)
                -- _print("inside lua "..t.x..","..t.y..","..t.z)
                -- _add(t)
                -- _ents[t.id]=t

                
                -- t.x=t.x+30
               

                
                -- t.y=t.y+5
                -- t.z=t.z+5
            end
        end
    end
    -- _print("count is"..c)
    -- _push(10.)
    -- _ents[0]={x=44}
    -- _print(70)
    -- _print(#_ents)
    -- _print("testo "..#_testo)
    -- _print("test index ".._testo[1])

    

    -- _print("we got ".._ents[1].x)

    -- _ents[1].x=_ents[1].x+1.
    -- _print("now we have ".._ents[1].x)

   

    -- for n=0,#_ents do
        -- _push(n)
        --  _ents[n].x=_ents[n].x+20
    -- end
end

-- function loop()
--     cam({0,0,0},{20*math.cos(rot),20*math.sin(rot),16.})
--     rot=rot+0.1
--     if rot>math.pi*2 then
--         rot=0
--     end
-- end

-- _bg(0,1,1,1)




-- local a=ent("chicken",0,0)
-- a.scale(2)
-- a.brain("walker")

-- rot=0.
-- _spawn(0,0,0)
local fellas={}
function _main()
    -- _ents={[0]={x=43}}
    -- _testo={53}
    -- _print('hi there')
    _bg(0,1,1,1)
    -- for i=0,10 do
    --     _spawn(i+5,0,2)
    -- end

    _print(69)

    for i=0,16 do
        for j=0,16 do
            _tile(0,i,j,-8)
        end
    end
    _tile_done()

    h=8
    c=0
    for i=-h,h do
        for j=-h,h do
            -- for k=-h,h do
            c=c+1
            t=_spawn(c%2==0 and "dog1" or "chicken",i,j,0)
            fellas[#fellas+1]=t
                -- _print("inside lua "..t.x..","..t.y..","..t.z)
                -- _add(t)
                -- _ents[t.id]=t

                
                -- t.x=t.x+30
               

                
                -- t.y=t.y+5
                -- t.z=t.z+5
            -- end
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

function _loop()
    for i = 1, #fellas do
        _walker(fellas[i])
        -- fellas[i].x=fellas[i].x+0.1
    end
end

-- function loop()
--     cam({0,0,0},{20*math.cos(rot),20*math.sin(rot),16.})
--     rot=rot+0.1
--     if rot>math.pi*2 then
--         rot=0
--     end
-- end

-- _bg(0,1,1,1)


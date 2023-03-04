-- Codex 2.1.0 "Avocado"
cursor = spawn('cursor', 0, 0, 0)
origin = { x = 0, y = 0, z = 0 }
zrot = 0
yrot = -tau / 8
distance = 12
cpos = { x = 0, y = 0, z = 0 }
mdown = false
cursor_grab_point = { x = 0, y = 0, z = 0 }

axis = "z"
vecs = {}
last_pos = { x = 0, y = 0, z = 0 }
hovered = -1


function main()
    sky()
    fill('f66')
    gui()
    local dot = nimg(1, 1)
    dot:pixel(0, 0, "0ff")
    tex("point", dot)
    local sz = 48
    local fade = nimg(sz, sz)
    local d = 0
    -- local ir = irnd(30)
    -- print("irnd " .. ir)
    for i = 0, sz - 1 do
        for j = 0, sz - 1 do
            -- d+= i*j
            -- if d>900 then
            --     d=0


            -- local d = flr(ir * j / 47) + 2
            -- if (i * i + j * j) % d == 0 then
            --     fade:pixel(i, j, "639bff") -- 639bff
            -- end

            -- if sin(i * j / 2) > .8 then
            --     -- if (i + j) % d == 0 then
            --     fade:pixel(i, j, "639bff") -- 639bff
            --     -- end
            -- end
            --
            if not (i > 5 and i < sz - 5 and j > 5 and j < sz - 5) and (i + j) % 3 == 0 then
                fade:pixel(i, j, "639bff") -- 639bff
            end
        end
    end
    tex("fade", fade)
    model("stage",
        {
            t = { "grid" },
            q = { { 0, 0, 0 }, { 0, 0, 1 }, { 1, 0, 1 }, { 1, 0, 0 } },
            -- u = { { 0, 0 }, { size, 0 }, { size, size }, { 0, size } }
        })

    model("panel",
        {
            t = { "fade" },
            q = { { 0, 0, 0 }, { 0, 0, 1 }, { 1, 0, 1 }, { 1, 0, 0 } },
        })
    xz = spawn("panel", -1, -1.1, -1)
    xz.scale = 3
    yz = spawn("panel", -1.1, -1, -1)
    yz.scale = 3
    yz.rz = tau / 4
    xy = spawn("panel", -1, -1, -1.1)
    xy.scale = 3
    xy.rx = -tau / 4

    stage = spawn("stage", -1, -1, -.1)
    stage.scale = 3
    stage.rx = -tau / 4

    model("test",
        {
            t = { "tri" },
            q = { { 0, 0, 0 }, { 0, 1, 0 }, { 1, 1, 0 }, { 1, 0, 0 } },
        })

    spawn("test", 0, 0, 0)
    -- cam { pos = { distance * cos(zrot), distance * sin(zrot), 12 }, rot = { zrot + tau / 4, yrot } }
end

function loop()
    if key("lshift") or key("rshift") then
        if key("w", true) or key("up", true) then
            yrot = yrot + tau / 32
        elseif key("s", true) or key("down", true) then
            yrot = yrot - tau / 32
        end

        if key("a", true) then
            zrot = (zrot + tau / 4)
            -- local r = zrot / (tau / 4)
            -- zrot = tau / 4 * (r - flr(r))
        elseif key("d", true) then
            zrot = (zrot - tau / 4)
            -- local r = zrot / (tau / 4)
            -- zrot = tau / 4 * (r - flr(r))
        end
    else
        if key("w") or key("up") then
            origin.x = origin.x - cos(zrot) * 0.1
            origin.y = origin.y - sin(zrot) * 0.1
        elseif key("s") or key("down") then
            origin.x = origin.x + cos(zrot) * 0.1
            origin.y = origin.y + sin(zrot) * 0.1
        end

        if key("a", true) then
            zrot = zrot + tau / 16
        elseif key("d", true) then
            zrot = zrot - tau / 16
        end
    end

    if key("lwin") then
        if key("z", true) then
            local f = del(vecs, #vecs)
            kill(f)
            print("new size" .. #vecs)
        end
    else
        if key("z", true) then
            set_axis_z()
        elseif key("x", true) then
            set_axis_x()
        elseif key("y", true) then
            set_axis_y()
        end
    end

    -- if key("space", true) then
    --     make_mesh()
    -- end
    -- example.z = example.z + rnd() * 0.1 - 0.05
    -- zrot+=0.02
    -- origin.x+=0.01
    cpos = {
        x = origin.x + distance * (cos(zrot) * cos(yrot)),
        y = origin.y + distance * (sin(zrot) * cos(yrot)),
        z = origin.z - distance * sin(yrot)
    }
    local m = mouse()


    local block_level = 0
    local vv = 18.
    local vx = m.vx * vv + cpos.x
    local vy = m.vy * vv + cpos.y
    local vz = m.vz * vv + cpos.z

    local f
    if axis == "z" then
        f = -(cpos.z) / (m.vz)
    elseif axis == "x" then
        f = -(cpos.x) / (m.vx)
    else
        f = -(cpos.y) / (m.vy)
    end

    local p = { x = cpos.x + f * m.vx, y = cpos.y + f * m.vy, z = cpos.z + f * m.vz }
    -- p.z +=block_level
    p.x = sx(p.x)
    p.y = sx(p.y)
    p.z = sx(p.z)
    -- local xx = flr(p.x + .5)
    -- local yy = flr(p.y + .5)
    -- local zz = flr(p.z + .5)
    cursor.x = p.x
    cursor.y = p.y
    cursor.z = p.z

    if m.m1 then
        if not mdown then
            mdown = true
            local s = spawn("point", p.x, p.y, p.z)
            s.scale = 1 / 8
            add(vecs, s)
            make_mesh()
        end
    elseif m.m2 then
        if not mdown then
            mdown = true
            cursor_grab_point = { x = p.x, y = p.y, z = p.z }
        else
            origin.x = origin.x + (cursor_grab_point.x - p.x)
            origin.y = origin.y + (cursor_grab_point.y - p.y)
            origin.z = origin.z + (cursor_grab_point.z - p.z)
        end
    elseif m.m3 then
        if not mdown then
            mdown = true
            cursor_grab_point = { x = m.x, y = m.y, z = zrot }
        else
            zrot = cursor_grab_point.z + (cursor_grab_point.x - m.x) * tau
            distance = distance - (cursor_grab_point.y - m.y) * 0.5
            -- origin.y = origin.y + (cursor_grab_point.y - m.y)
        end
    else
        mdown = false
    end

    if m.scroll ~= 0 then
        distance = distance + m.scroll * 0.1
        if distance < 0.1 then
            distance = 0.1
        end
    end

    cursor_check(p)

    clr()
    text((flr(p.x * 100) / 100) .. ", " .. (flr(p.y * 100) / 100) .. ", " .. (flr(p.z * 100) / 100), m.x - .1, m.y - .1)
    cam { pos = { cpos.x, cpos.y, cpos.z }, rot = { zrot + tau / 2, yrot } }
end

function make_mesh()
    local v = {}
    for i = 1, #vecs do
        v[i] = { vecs[i].x, vecs[i].y, vecs[i].z }
        -- kill(vecs[i])
    end
    model("test", { q = v, t = { "tri" } })
end

function sx(x)
    return flr(x * 10) / 10
end

function set_axis_z()
    axis = "z"
    yrot = -tau / 4
    zrot = -tau / 4
    origin = { x = 0, y = 0, z = 0 }
    -- xy.asset = "stage"
    -- xz.asset = "panel"
    -- yz.asset = "panel"

    stage.rx = -tau / 4
    stage.rz = 0
    stage.z = -.1
    stage.x = -1
    stage.y = -1
end

function set_axis_x()
    axis = "x"
    yrot = 0
    zrot = 0
    origin = { x = 0, y = 0, z = 0 }
    stage.rx = 0
    stage.rz = tau / 4
    stage.z = -1
    stage.y = -1
    stage.x = -.1
    -- xy.asset = "panel"
    -- xz.asset = "panel"
    -- yz.asset = "stage"
end

function set_axis_y()
    axis = "y"
    yrot = 0

    zrot = tau / 4
    origin = { x = 0, y = 0, z = 0 }
    stage.rx = 0
    stage.rz = 0
    stage.z = -1
    stage.y = -.1
    stage.x = -1
    -- xy.asset = "panel"
    -- xz.asset = "stage"
    -- yz.asset = "panel"
end

--- @param c {x:number,y:number,z:number}
function cursor_check(c)
    if last_pos.x == c.x and last_pos.y == c.y and last_pos.z == c.z then
        return
    else
        hovered = -1
        for i = 1, #vecs do
            local v = vecs[i]
            if v.x == c.x and v.y == c.y and v.z == c.z then
                v.scale = 1 / 4
                hovered = i
            else
                v.scale = 1 / 8
            end
        end
    end
    last_pos = last_pos or c
end

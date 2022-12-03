math.randomseed(os.time())
crt({
    dark = 99.,
    low = 0.1,
    high = 0.9,
    flatness = 4.,
    curvature = 0.8,
    resolution = 720,
    glitch = 4.
})
-- asong = { "F", "_" }
-- last = ""

-- layout = { "" }

bars = {}
bar_notes = {}
-- note_set = { "C", "Cs", "D", "Ds", "E", "F", "Fs", "G", "Gs", "A", "As", "B" }

iter_notes = 1
sound_counter = 0
out = {}


bar_ui_size = 0.8
bar_ui_size2 = 1. - bar_ui_size


mouse_once = false

last_notes = {}
last_instr = {}




-- e c - e f - - f
--  G - - A _ G e c
sky()
fill("000")

function rando(min, max)
    if max == nil then
        max = min
        min = 0
    end
    return min + math.random() * (max - min)
end

pps = {}
function main()
    for i = 1, 100 do
        e = {
            h = rando(10.),
            pp = spawn("jumpers" .. flr(rando(0, 4)), rando(-12., 12.), rando(8, 40), rando(0., 4.)),
            vel = 0.
        }
        -- e.pp:anim("Idle")
        pps[#pps + 1] = e
    end

    pps[1].pp.x = 2.
    pps[1].pp.y = 0.
    pps[1].pp.z = -.9

    local poof = gimg("poofy")


    for i = 1, 12 do
        img(poof, rnd(), rnd() / 3.)
        --     spawn("poofy", rnd(-12., 12.), 18, rnd(1., 8.))
    end
    gui()

    for i = -10, 10 do
        for j = 0, 40 do
            tile("colored" .. flr(rando(0, 4)), i, j, -2)
        end
    end

    -- print("note_set", table.concat(note_set, ", "))

    for i = 1, bar_ui_count do
        if i % 2 == 0 then
            bars[i] = 0.
        else
            bars[i] = 38. / 440.
        end
        last_instr[i] = bars[i]
        last_notes[i] = bars[i]
        bar_notes[i] = check_bar(i)
    end

    make_ui_buttons()

    send_instr()
    -- instr(2., noise_wave())


end

-- sounder = .1
svel = .02

sounder = 1.
stime = 0.
maxi = 0
ss = 0
ddir = 0.04
d = 0.
lx = 0.
ly = 0.

camera = {
    x = 0.,
    y = 0,
    z = 0
}
overlay = ""
function check_bar(bi)

    -- local v = 1 + flr(bars[bi] * (#note_set - 1))
    -- return note_set[v]
    return (bars[bi] * 440.)
end

function loop()

    overlay = ""
    for i, e in ipairs(pps) do
        if e.pp.z > -1 then
            e.vel = e.vel - 0.01
            e.pp.z = e.pp.z + e.vel
        else
            e.vel = -e.vel
            e.pp.z = -0.999
        end
    end
    clr()

    if key("space", true) then
        activate()
    end

    local m = mouse()
    if m.m1 then
        -- if mouse_once then
        --     mouse_once = false
        -- end
    else
        mouse_once = true
    end

    -- print("mouse " .. m.x)
    if m.y > 0.15 then
        ui_bars(m)
    end

    draw_bars()

    draw_buttons(m)

    d = d + ddir
    if d > 1 then
        ddir = -ddir
    elseif d < 0 then
        ddir = -ddir
    end

    vr = 2. + d

    ss = ss + 0.1
    sx = math.cos(ss) / vr + 0.5
    sy = math.sin(ss) / 20. + 0.9

    if lx ~= 0 then
        line(lx, ly, sx, sy)
    end

    lx = sx
    ly = sy


    if #overlay > 0 then
        rect(m.x + 1 / 64, m.y - 1 / 16, 10 * #overlay, 10, "fff")
        text(overlay, m.x, m.y - 1 / 16)
    end

    campos(camera.x, camera.y, camera.z)

end

function clamp(n, min, max)
    if n < min then
        return min
    elseif n > max then
        return max
    else
        return n
    end
end

function make_square()
    for i = 1, bar_ui_count do
        if i % 2 == 0 then
            bars[i] = 0.
        else
            bars[i] = 38. / 440.
        end
        bar_notes[i] = check_bar(i)
    end
end

function make_noise()
    for i = 1, bar_ui_count do
        bars[i] = math.random()
        bar_notes[i] = check_bar(i)
    end
end

function activate()
    silence()
    if mode_type then
        print("Notes")
        send_notes()
    else
        print("Instr")
        send_instr()
    end
end

function send_instr()
    if half_enabled then
        print "half"
    else
        print "full"
    end
    instr(bars, half_enabled)
    sound(440., 2.)
end

function send_notes()
    out = {}

    local dur = song_speed / 40.
    -- print("dur" .. dur)
    for i = 1, #bars do
        if bars[i] > 0. then
            out[#out + 1] = { bar_notes[i], dur } --octave4[bar_notes[i]]
        else
            out[#out + 1] = { 0., dur }
        end
    end
    -- print "OUT"
    -- out = square_wave()
    -- print("out:" .. table.concat(out, ","))

    -- instr(2., out)
    song(out)
end

function draw_bars()
    local bb = (bar_ui_count + 2)
    local w = 1 / (bar_ui_count + 3)
    -- print("bars" .. #bars)
    local c = "BF7298"
    if mode_type then
        c = "85C0FF"
    end
    for i = 1, #bars do
        local h = bars[i]
        local x = i / bb
        if h == 0. then
            rect(x, 0.95, w, 0.05, "006")
        else
            rect(x, (1. - h) * bar_ui_size + bar_ui_size2, w, h * bar_ui_size, c)
            -- text(flr(bar_notes[i]), x - 8 / 320., 1. - h - 8 / 240.)
        end

    end
end

function ui_bars(m)
    local bb = (bar_ui_count + 2)
    local w = 1 / bb
    for i = 1, #bars do
        local x = i / bb

        if m.x > x and m.x < x + w then
            if m.m1 then
                bars[i] = clamp(1. - m.y, 0., bar_ui_size) / bar_ui_size

                bar_notes[i] = check_bar(i)
                overlay = "" .. flr(bar_notes[i]) .. "hz"
            elseif m.m2 then
                bars[i] = 0.
                bar_notes[i] = 0. --""
            end

            -- print("bar is " .. m.y)
        end

    end
end

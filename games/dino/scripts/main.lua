math.randomseed(os.time())
crt({
    dark = 25.,
    low = 0.1,
    high = 0.9,
    flatness = 2.,
    curvature = 0.8,
    resolution = 720,
    glitch = 4.
})
notes = {
    C = 130.81,
    Cs = 138.59,
    D = 146.83,
    Ds = 155.56,
    E = 164.81,
    F = 174.61,
    Fs = 185.00,
    G = 196.00,
    Gs = 207.65,
    A = 220.00,
    As = 233.08,
    B = 246.94,
    Z = -1
}
-- song = {"G", "E", "G", "G", "E", "G", "A", "G", "F", "E", "D", "E", "F"}
last = ""
song = {"E", "C", "C", "E", "F", "F", "F", "F", "G", "G", "G", "A", "A", "G", "E", "C", "Z"}
-- e c - e f - - f
--  G - - A _ G e c
bg(0.1, 1., 1.)
function rnd(min, max)
    if max == nil then
        max = min
        min = 0
    end
    return min + math.random() * (max - min)
end

pps = {}
function main()
    for i = 1, 300 do
        e = {
            h = rnd(10.),
            pp = spawn("dino", rnd(2, 40), rnd(-30., 30.), rnd(0., 4.)),
            vel = 0.
        }
        -- e.pp:anim("Idle")
        pps[#pps + 1] = e
    end

    pps[1].pp.x = 2.
    pps[1].pp.y = 0.
    pps[1].pp.z = -.9

    for i = 1, 50 do
        spawn("poofy", 18, rnd(-12., 12.), rnd(1., 8.))
    end
    for i = -40, 40 do
        for j = -1, 40 do
            h = math.random()
            if h > 0.75 then
                t = "ground18"
            elseif h > 0.5 then
                t = "ground17"
            elseif h > 0.25 then
                t = "ground10"
            else
                t = "ground9"
            end

            tile(t, j, i, -2) -- 9 10 17 18
        end
    end

end

-- sounder = .1
svel = .02

sounder = 1.
stime = 0.
function loop()
    for i, e in ipairs(pps) do
        if e.pp.z > -1 then
            e.vel = e.vel - 0.01
            e.pp.z = e.pp.z + e.vel
        else
            e.vel = -e.vel
            e.pp.z = -0.999
        end
    end

    stime = stime + 0.1
    if stime > 1. then
        stime = 0.
        sounder = sounder + 1
        if sounder > #song then
            sounder = 1
        end
    end
    newnote = song[sounder]
    if newnote ~= last then
        -- log(newnote)
        sound(notes[newnote])
    end
    last = newnote

    -- sounder = sounder + svel
    -- if sounder > high then
    --     sounder = high
    --     svel = -svel
    -- elseif sounder < low then
    --     sounder = low
    --     svel = -svel
    -- end

    -- sound(math.floor(sounder / .02) * .02)
    -- sound(sounder)
end

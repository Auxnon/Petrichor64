-- Codex 3.0.0 "Artichoke"
sky:fill('FF5')

function main()
    example = make('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)

    local im = nimg(16, 16)
    im:fill('00F')
    tex('generated', im)
    generated = make('generated', rnd() * 3. - 1.5, 10, rnd() * 3. - 1.5)
    spin = 0

    for i = 0, 6 do
        for j = 0, 6 do
            local t = 'example'
            if (j + i + 1) % 2 == 0 then
                t = 'generated'
            end
            tile(t, i - 3, 9 + j, -3)
        end
    end

    cam { pos = { 0, 0, 0 }, rot = { tau / 4, 0 } }
    cout 'main runs once everything has loaded'
    local back = nimg(1, 1)
    back:fill('444')
    tex('back',back)
    -- the editor's 320x240 canvas, registered as 'canvas' (see edit.lua)
    edit_init()
    -- vertical 4x3 quad in the XZ plane (Z-up), centered, facing the camera down +y.
    -- Explicit UVs: the quad form's estimated UVs fit the long side to 1 and would
    -- crop a non-square canvas.
    mod("canvasquad", {
        t = "canvas",
        q = { { -2, 0, -1.5 }, { 2, 0, -1.5 }, { 2, 0, 1.5 }, { -2, 0, 1.5 } },
        u = { { 0, 1 }, { 1, 1 }, { 1, 0 }, { 0, 0 } },
    })
    pane=make("canvasquad", 0, 10, 0)
    overlay=make("canvasquad", 0,- 0.1, 0)
    pane.tex='back'
    lot(pane,overlay)
    -- pane.ry=tau*.1
    pane.rz+=tau*.1
    edit_attach(overlay, pane)
end

function loop()
    example.x = example.x + rnd(-0.05, 0.05)
    example.z = example.z + rnd(-0.05, 0.05)
    spin = spin + 0.1
    generated.z = generated.z + cos(spin) * .04
    generated.x = generated.x + sin(spin) * .04
    -- pane.rz+=0.01
    edit_tick()
end

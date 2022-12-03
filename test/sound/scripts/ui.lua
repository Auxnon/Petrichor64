buttons = {}

mode_type = true
half_enabled = false
song_speed = 1
bar_ui_count = 16

function make_button(x, y, w, h, states, reference)
    local midx = (w / 2.) - (#states[1][1] * 12)
    local midy = (h / 2.) - (4)

    local state_on = 1

    if not type(_G[reference]) == "function" then
        state_on = _G[reference]
    end

    local b = {
        x = x,
        y = y,
        w = w,
        h = h,
        txt = txt,
        color = color,
        color_over = color_over,
        color_down = color_down,
        down = false,
        over = false,
        midx = flr(midx),
        midy = flr(midy),
        state_on = state_on,
        is_toggle = type(_G[reference]) == "boolean",
        states = states,
        reference = reference
    }


    buttons[#buttons + 1] = b
end

function draw_button(b, m)
    local mx = m.x * 320.
    local my = m.y * 240.

    local state = b.states[1]
    if b.is_toggle then
        if b.state_on then
            state = b.states[2]
        end
    else
        state = b.states[b.state_on]
    end

    if mx > b.x and mx < b.x + b.w and my > b.y and my < b.y + b.h then
        b.over = true
        if m.m1 then
            b.down = true
            if mouse_once then
                mouse_once = false
                if b.is_toggle then
                    b.state_on = not b.state_on
                else
                    b.state_on = b.state_on + 1
                    if b.state_on > #b.states then
                        b.state_on = 1
                    end
                end
                if type(_G[b.reference]) == "function" then
                    _G[b.reference](b.state_on)
                else
                    _G[b.reference] = b.state_on
                end
            end
        else
            b.down = false
        end
    else
        b.over = false
        b.down = false
    end

    local c = state[2]
    if b.down then
        c = state[3]
    elseif b.over then
        c = state[4]
    end

    rect(b.x, b.y, b.w, b.h, c)
    text(state[1], b.x + b.midx, b.y + b.midy)
end

function draw_buttons(m)
    --
    for i = 1, #buttons do
        draw_button(buttons[i], m)
    end
end

function make_ui_buttons()
    local b1 = "85C0FF"
    local b2 = "47A0FF"
    local b3 = "00D3FF"

    local r1 = "BF7298"
    local r2 = "FF7DBD"
    local r3 = "BB0962"

    local g1 = "9bd6a1"
    local g2 = "49dc58"
    local g3 = "9effc5"

    local y1 = "f1ff9e"
    local y2 = "e9ff65"
    local y3 = "f3ec47"
    make_button(4, 4, 50, 20, { { "Note", b1, b2, b3 }, { "Inst", r1, r2, r3 } }, "change_mode")
    make_button(58, 4, 40, 20, { { " 1 ", b1, b2, b3 }, { "1/N", r1, r2, r3 } }, "half_enabled")
    make_button(102, 4, 40, 20,
        { { "x1", b1, b2, b3 }, { "x2", g1, g2, g3 }, { "x4", y1, y2, y3 }, { "x8", r1, r2, r3 } }
        , "change_speed")

    make_button(146, 4, 40, 20,
        { { "16", b1, b2, b3 }, { "32", g1, g2, g3 }, { "64", r1, r2, r3 } }
        , "bar_change")

    make_button(190, 4, 30, 20,
        { { "_", y1, y2, y3 } }, "make_square")

    make_button(224, 4, 30, 20,
        { { "R", y1, y2, y3 } }, "make_noise")


    make_button(258, 4, 50, 20,
        { { "send", g1, g2, g3 } },
        "activate")

    -- make_button(0.25, 0.05, 0.15, 0.1, { { "Half Off", b1, b2, b3 }, { "Half On", r1, r2, r3 } }, "half_enabled")
    -- make_button(0.45, 0.05, 0.15, 0.1, { { "Mode 1", b1, b2, b3 }, { "Mode 2", r1, r2, r3 } })

end

function change_mode()
    mode_type = not mode_type
    if mode_type then
        last_instr = bars
        bars = last_notes
    else
        last_notes = bars
        bars = last_instr

    end

    for i = 1, bar_ui_count do
        bar_notes[i] = check_bar(i)
    end
end

function change_speed(s)
    if s == 1 then
        song_speed = 1
    elseif s == 2 then
        song_speed = 2
    elseif s == 3 then
        song_speed = 4
    elseif s == 4 then
        song_speed = 8
    end
end

function bar_change(v)
    local pre_bar = bars
    -- table.remove(bars)
    if v == 1 then
        bar_ui_count = 16
    elseif v == 2 then
        bar_ui_count = 32
    elseif v == 3 then
        bar_ui_count = 64
    end

    if #pre_bar > bar_ui_count then
        table.remove(bars)
        for i = 1, bar_ui_count do
            bars[i] = pre_bar[i]
        end

    elseif #pre_bar < bar_ui_count then
        for i = #pre_bar, bar_ui_count do
            bars[i] = 0.
        end
    end

    table.remove(bar_notes)
    for i = 1, bar_ui_count do
        bar_notes[i] = check_bar(i)
    end

end

function die_roll()
    local r = irnd(1, 7)
    print("roll" .. r)
    if r == 1 then
        die.ry = 0
        die.rz = 0
        die.rx = 0
    elseif r == 2 then
        die.ry = 0
        -- die.rz = tau / 4
        die.rz = 0
        die.rx = tau / 2
    elseif r == 3 then -- right
        die.rx = 0
        die.rz = 0
        die.ry = -tau / 4
    elseif r == 4 then
        die.rx = 0
        die.rz = 0
        die.ry = tau / 4
    elseif r == 5 then
        die.ry = 0
        die.rx = tau / 4
        die.rz = 0
    elseif r == 6 then -- right
        die.rx = tau / 4
        die.rz = tau / 2
    end


end

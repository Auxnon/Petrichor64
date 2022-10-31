ZOM_RATES = { 960, 480, 240, 120 }
GUY_RATES = { 1920, 960, 480, 240 }
ZOM_RATE = ZOM_RATES[1]
GUY_RATE = ZOM_RATES[1]
function difficulty_loop()
    if survive_timer > 180 then
        ZOM_RATE = ZOM_RATES[4]
        GUY_RATE = ZOM_RATES[4]
    elseif survive_timer > 120 then
        ZOM_RATE = ZOM_RATES[3]
        GUY_RATE = ZOM_RATES[3]
    elseif survive_timer > 60 then
        ZOM_RATE = ZOM_RATES[2]
        GUY_RATE = ZOM_RATES[2]
    else
        ZOM_RATE = ZOM_RATES[1]
        GUY_RATE = ZOM_RATES[1]
    end
end

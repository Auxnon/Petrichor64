-- Codex 3.0.0 "Artichoke"
example = make('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
sky:fill('F5F')
message = ""
output = ""
input = ""
check_time = 0
function main()
    cout 'start connection!'
    test = conn '127.0.0.1:6142'
    cout 'connected!'
    test:send 'hello'
end

function loop()
    if test then
        example.x = example.x + rnd() * 0.1 - 0.05
        example.z = example.z + rnd() * 0.1 - 0.05
        local t = cin()
        if t then
            message = message .. t
        end
        if key 'return' and message:len() > 0 then
            print('sending message' .. message)
            output = message
            test:send(message)
            redraw()
            message = ''
        end
        local incoming = test:recv()
        if incoming then
            print('received message' .. incoming)
            input = incoming
            redraw()
        end
        if check_time <= 0 then
            check_time = 200
            local r = test:test()
            if r then
                print('connection error ' .. r)
                test:kill()
                test = nil
            else
                print 'connection ok'
            end
        else
            check_time = check_time - 1
        end
    else
        if check_time <= 0 then
            print 'remaking connection'
            test = conn '127.0.0.1:6142'
            local r = test:test()
            if r then
                print('connection error ' .. r)
                test:kill()
                test = nil
                check_time = 400
            end
        else
            check_time = check_time - 1
        end
    end
end

function draw(w, h)
    print("redraw", w, h)
    redraw()
end

function redraw()
    gui:clr()
    gui:text(output, 40, 40, '00f')
    gui:text(input, 40, 60, 'ff0')
end

-- example = make('example', rnd() * 3. - 1.5, 12, rnd() * 3. - 1.5)
fill('FFF')

function main()
    cout('main runs once everything has loaded')
    local test = 1
    if type(test) == "number" then
        cout('true is a boolean')
    else
        cout('true is not a boolean')
    end
    print("ugh" .. type(test))
    cout(ent)
    ent = make("example", 0, 0, 0)
end

function loop()
    -- example.x = example.x + rnd() * 0.1 - 0.05
    -- example.z = example.z + rnd() * 0.1 - 0.05
end

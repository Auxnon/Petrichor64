math.randomseed(os.time())
example = spawn('example', 12, math.random() * 3. - 1.5, math.random() * 3. - 1.5)
bg(1, 1, .4, 1)
function main()
    log('main runs once everything has loaded')
end
function loop()
    example.x = example.x + math.random() * 0.1 - 0.05
    example.y = example.y + math.random() * 0.1 - 0.05
end
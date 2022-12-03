example = spawn('example', rando() * 3. - 1.5, 12, rando() * 3. - 1.5)
buffer = ""
text_cursor = { x = 0, y = 0 }
cursor = { x = 0, y = 0 }
lines = {}
sky()
fill(.4, 1., 0.2, 1.)
gui()

curs = { "°", "±", "²" }
cursi = 1
matchers = {}
suggestions = {}

function main()
    log('main runs once everything has loaded')
    -- help():pairs()
    for key, value in pairs(help()) do
        matchers[#matchers + 1] = key
        print(key .. ":" .. value)
    end
end

function loop()
    if key("a") then
        example.x = example.x + rando() * 0.1 - 0.05
        example.z = example.z + rando() * 0.1 - 0.05
    end
    if key("del", true) or key("back", true) then
        -- print(buffer:sub(1, -2))
        local ind = get_ind()
        buffer = buffer:sub(1, ind - 2) .. buffer:sub(ind, -1)
        move_text_cursor(-1)
        refresh()
    elseif key("return", true) then
        print "yeah return"
        local ind = get_ind()
        local line = buffer:sub(1, ind - 1)
        buffer = buffer:sub(ind, -1)
        table.insert(lines, line)
        text_cursor.x = 0
        text_cursor.y = text_cursor.y + 1
        refresh()
    elseif key("left", true) then
        move_text_cursor(-1)
        refresh()
    elseif key("right", true) then
        move_text_cursor(1)
        refresh()
    end

    if key("tab", true) and #suggestions then
        local range = get_word_range()
        local diff = #suggestions[1] - (range[2] - range[1])
        move_text_cursor(diff)
        buffer = buffer:sub(1, range[1] - 1) .. suggestions[1] .. buffer:sub(range[2], -1)
        refresh()
    else
        h = input()
        if #h > 0 then
            -- insert string
            local ind = get_ind()
            if key("lshift") or key("rshift") then
                h = shift_keys(h)
            end
            buffer = buffer:sub(1, ind - 1) .. h .. buffer:sub(ind)
            -- buffer = buffer .. h
            move_text_cursor(#h)
            refresh()
        end
    end

end

function refresh()
    clr()
    draw_cursor()
    word_wrap()
end

function move_text_cursor(amount)

    text_cursor.x = text_cursor.x + amount
    if amount > 0 then
        if text_cursor.x > 16 then
            text_cursor.y = text_cursor.y + flr(text_cursor.x / 16)
            text_cursor.x = text_cursor.x % 16
        end
    else
        if text_cursor.x < 0 then
            xx = ceil(abs(text_cursor.x / 16.))
            text_cursor.y = text_cursor.y - xx

            text_cursor.x = text_cursor.x % 16
            if text_cursor.y < 0 then
                text_cursor.y = 0
                text_cursor.x = 0
            end
        end

        print(text_cursor.x)
    end
end

function get_ind()
    return text_cursor.y * 16 + text_cursor.x + 1
end

function draw_cursor()

    -- text(curs[cursi], cursor.x * 9, cursor.y * 16 + 4)
    text(curs[cursi], text_cursor.x * 9, text_cursor.y * 16)

    cursi = cursi + 1
    if cursi > #curs then cursi = 1 end
end

function word_wrap()
    -- buffer:find("\n", 1, true)
    local lcount = flr(#buffer / 16) + 1
    local last = nil
    for i = 1, lcount do
        local l = buffer:sub(i * 16 - 15, i * 16)
        lines[i] = l
        -- cursor.y = i
        last = l
        text(l, 0, i * 16 - 16)
    end

    local range = get_word_range()
    local l = buffer:sub(range[1], range[2])
    print("cut is " .. l)
    local res = compare(l)
    print("-----")
    suggestions = res
    if #suggestions > 0 then
        list_suggestions(suggestions)
    end
end

function get_word_range()
    local ind = get_ind()
    local start = buffer:sub(1, ind)
    local start_ind = start:find("%w+$")
    if start_ind then
        start = start:sub(start_ind)
    else
        start = ""
    end
    local endd = buffer:sub(ind)
    local end_ind = endd:find("%W")
    if end_ind then
        endd = endd:sub(1, end_ind - 1)
    else
        endd = ""
    end
    return { ind - #start, ind + #endd }

    -- local bb = buffer:sub(1, ind)
    -- local bb = bb:reverse()
    -- local start = (bb:find(" ", 1, true) or 0) + 1
    -- local endd = (buffer:find(" ", ind, true) or #buffer + 1) - 1
    -- start = ind - start
    -- local l = buffer:sub(start, ind)
    -- print("start is " .. start .. " end is " .. endd)
    -- return { start, ind }
end

-- convert string characters to shifted characters
---@param key string
---@return string
function shift_keys(key)
    key = key:gsub("1", "!")
    key = key:gsub("2", "@")
    key = key:gsub("3", "#")
    key = key:gsub("4", "$")
    key = key:gsub("5", "%")
    key = key:gsub("6", "^")
    key = key:gsub("7", "&")
    key = key:gsub("8", "*")
    key = key:gsub("9", "(")
    key = key:gsub("0", ")")
    key = key:gsub("%-", "_")
    key = key:gsub("=", "+")
    key = key:gsub("%[", "{")
    key = key:gsub("%]", "}")
    key = key:gsub("\\", "|")
    key = key:gsub(";", ":")
    key = key:gsub("'", "\"")
    key = key:gsub(",", "<")
    key = key:gsub("%.", ">")
    key = key:gsub("/", "?")
    return key





    -- key:gsub(".", function(k)
    --     if k == "1" then return "!" end
    --     if k == "2" then return "@" end
    --     if k == "3" then return "#" end
    --     if k == "4" then return "$" end
    --     if k == "5" then return "%" end
    --     if k == "6" then return "^" end
    --     if k == "7" then return "&" end
    --     if k == "8" then return "*" end
    --     if k == "9" then return "(" end
    --     if k == "0" then return ")" end
    --     if k == "-" then return "_" end
    --     if k == "=" then return "+" end
    --     if k == "[" then return "{" end
    --     if k == "]" then return "}" end
    --     if k == ";" then return ":" end
    --     if k == "'" then return "\"" end
    --     if k == "," then return "<" end
    --     if k == "." then return ">" end
    --     if k == "/" then return "?" end
    --     if k == "`" then return "~" end
    --     if k == "\\" then return "|" end
    --     return k
    -- end)

end

--- test if string partially matches item in matchers table
---@param str string
function compare(str)
    local trimmed = str:match("^%s*(.-)%s*$")
    -- print(" match thsi long " .. #matchers)
    local list = {}
    for i = 1, #matchers do
        ---@type string
        local m = matchers[i]
        -- print("compare " .. m .. " to " .. trimmed)
        if m:find(trimmed, 1, true) then
            list[#list + 1] = m
        end
    end
    return list
end

function list_suggestions(list)
    local x = text_cursor.x * 9
    for i = 1, #list do
        local y = (text_cursor.y + i) * 16
        local word = list[i]
        rect(x + 8, y, #word * 9, 16, 1., 0, 0.8)
        text(word, x, y)
    end
end

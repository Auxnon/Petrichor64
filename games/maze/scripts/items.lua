--- @type Entity[]
items = {}
--- @type Entity | nil
item_grabbed = nil


--- @class Vector
--- @field x number
--- @field y number
--- @field z number

--- @param item Entity
--- @param pos Vector
function within_range(item, pos)
    if pos.x > item.x - 1 and pos.x < item.x + 1 then
        if pos.y > item.y - 1 and pos.y < item.y + 1 then
            return true
        end
    end
end

function add_item(e)
    table.insert(items, e)
end

function check_items()
    if mouse_once then
        item_grabbed = nil
        for i, item in ipairs(items) do
            if within_range(item, cursor) then
                if item_grabbed then
                    if item.z > item_grabbed.z then
                        item_grabbed = item
                    end
                else
                    item_grabbed = item
                end
            end
        end
    else
        for i, item in ipairs(items) do
            if item ~= item_grabbed and within_range(item, cursor) then
                cursor.z = cursor.z + 1
            end
        end
        if item_grabbed then
            cursor.z = cursor.z + 2
            item_grabbed.x = cursor.x
            item_grabbed.y = cursor.y
            item_grabbed.z = cursor.z
        end


    end
end

function item_release_check()
    if item_grabbed then
        local max_z = 0
        for i, item in ipairs(items) do
            if item ~= item_grabbed and within_range(item, cursor) then
                if item.z > max_z then
                    max_z = item.z
                end
            end
        end
        item_grabbed.z = max_z + 1
        if item_grabbed == die then
            die_roll()
        end
    end
    item_grabbed = nil
end

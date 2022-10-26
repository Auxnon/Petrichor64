Dict = {}


function Dict.new(self, t)
    local dict = {}
    dict._list = {}
    dict.hash = {}
    dict.iterator = 1

    ---@type fun(self:table,e:any)
    function dict:add(e)

        local e2
        if type(e) ~= "table" then
            e2 = {}
            e2[1] = e
        else
            e2 = e
        end

        table.insert(self._list, 1, e2)
        self.hash[self.iterator] = e2
        e2.id = self.iterator
        self.iterator = self.iterator + 1
        return e2
    end

    ---@type fun(self:table,e:table)
    function dict:remove(e)
        if e.id ~= nil then
            dict.hash[e.id] = nil
            for i = 1, #dict._list do
                if dict._list[i].id == e.id then
                    table.remove(self._list, i)
                    break;
                end
            end
        end
    end

    ---@type fun(self:table,i:integer):table|nil
    function dict:get(i)
        print("get it " .. i)
        local e = self.hash[i]
        print("we got " .. type(e))
        return e
    end

    ---@type fun(self:table):table
    function dict:list()
        return self._list
    end

    if type(t) == "table" then
        for k, v in pairs(t) do
            dict:add(v)
        end
    end
    return dict
end

### help

_list commands_

```lua
---@type fun(expand?:boolean):table
function help(expand)
```

List all commands in a key pair format, such that resulting table you can find `table["cos"]` is the description for the `cos` command. Passing in a true boolean will have the resulting table list an array including the annotation comments used for the `ignore.lua` file a LSP consumes for typing and function defintions.

```lua
help()["cos"] -- "Cosine value"
help(true)["cos"] -- {"Cosine value", "---@param f number \n ---@return number\n ..."}
```

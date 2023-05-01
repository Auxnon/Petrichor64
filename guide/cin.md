### cin

_character input_

```lua
--- @type fun():string
function cin()
```

Returns all detected key strokes on an english keyboard related that occured between the current and last game loop and returns it as a string. Will naturally ignore irrelvent characters such as escape or function keys that cannot represented as a string. Hard coded keys

```lua
message=message+cin()
```

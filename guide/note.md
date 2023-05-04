### note

_play note_

```lua
---@type fun(freq:number, length?:number)
function note(freq,length)
```

**WIP**

Play a note of the specified frequency in hertz with the current instrument of an optional length. a parameter of 440.0 for instance represents 440 hertz or A above middle C in the 4th harmonic. All sound occupies it's own thread independent of all other processes.

```lua
note(110.0) -- play A1
```

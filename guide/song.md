### song

_asynchronous note sequence_

```lua
---@type fun(data:{freq:number,len?:number}[] | number[])
function song(data)
```

**WIP**

Play a series of notes similar to [note](#note) command. Can pass in an array of numbers for the frequencies, or an array of 2 numbers as an array nested.

As sound is an independent thread, sending notes as a sequence with a single command is more likely to stream without break or interruption.

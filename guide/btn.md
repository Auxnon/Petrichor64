### btn

_check button_

```lua
---@type fun(button:string):boolean
function btn(button)
```

Provides the ability to check for button presses on a connected gamepad. The library, gilrs, is fairly expansive and should be plug and play on any operating system, including SteamOS. Currently there isn't any easy way of determining a good keyboard layout for this so the best option is to check for both a button and a key press with a logical `OR`.

- **south** the southern button on right side
- **north** the northern button on the right side
- **east** the eastern button on the right side
- **west** the western button on the right side
- **dleft** the left directional button
- **dright** the right directional button
- **dup** the up directional button
- **ddown** the down directional button
- **laxisx** the left analog stick's x axis
- **laxisy** the left analog stick's y axis
- **raxisx** the right analog stick's x axis
- **raxisy** the right analog stick's y axis
- **lstick** the left analog stick's center button
- **rstick** the right analog stick's center button
- **rtrigger** the right trigger
- **ltrigger** the left trigger
- **rbumper** the right bumper
- **lbumper** the left bumper
- **start** the start button
- **select** the select button

```lua
if btn("south") then
    jump()
end
```

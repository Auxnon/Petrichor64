## attr

_set attribute_

```lua
---@type fun(table:table)
function attr(table)
```

Tweak monitor properties, lock mouse, disable console, etc. If there's a global attribute to be set to define, it's likely by this method. Attributes can be set by a number or a boolean. A boolean is interpreted as 1 or 0, and a number can be evalated as true if not 0, etc. Unless specified otherwise they are single values not arrays.

**Monitor**

- resolution: number - CRT screen resolution works off a single value to auto determine aspect ratio based on window size. Can differ from render pipeline's resolution.
- curvature: number
- flatness: number
- dark: number
- bleed: number
- glitch: number[] - 1-3 number array
- high: number
- low: number
- modernize: boolean - swap out CRT for an LED monitor

**Window**

- title: string - set the window's name
- fullscreen: boolean - fullscreen the window, can still be windowed again with ctrl/cmd + enter
- mouse_grab: boolean - First person style mouse grab, ungrabs while console is open
- size: integer[] - Integer array. Force the window to a certain size, this is logically based and not physically

**Misc**

- lock: boolean - Prevent the console from being open. Technically an anti-cheat but probably not
- fog: number - sets a simple draw distance based fog that fades into the skybox raster

```lua
attr({title="Your Game Or App Name Goes Here"})
attr{modernize=0} -- try default crt monitor settings
attr{fullscreen=true}
attr{fog=100.0}
```

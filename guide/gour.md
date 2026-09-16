## gour

_enable/disable Gouraud (per-vertex) shading_

```lua
---@type fun(on?: boolean)
function gour(on)
```

Switches how [lum](#lum)'s light gets applied. Off by default (today's
per-fragment "smooth" shading — every pixel computes its own lighting). When
on, lighting is computed once per **vertex** and interpolated across each
triangle instead — the classic PS1/N64 look: cheaper, and slightly wobbly on
low-poly meshes since the light can't curve within a single triangle.

The shadow map ([shdw](#shdw)) always stays per-fragment either way, so shadow
edges don't get any blockier than the shadow map already is.

```lua
gour()       -- Gouraud on
gour(false)  -- back to per-fragment "smooth"
```

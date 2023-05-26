## lot

_group entity_

```lua
---@type fun(parent:entity, child:entity)
function lot(parent,child)
```

Groups an entity on another, offsetting on it's current position and rotation. The parent entity is reordered on the entity stack to always be called first for it's transform matrix. Keep in mind that cyclic grouping is impossible, and trying to set a child as a parent to a it's own parent anywhere on the grouping tree will reorder it and this create unpredictable results. This doesn't fail in other words just keep in mind it could get wacky if you start grouping wildly.

```lua
parent=make()
child=make()
lot(parent,child)
parent.x=10 -- child will appear at 10 X but still have relative X of 0
```

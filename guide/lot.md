## lot

_group entity_

```lua
---@type fun(parent:entity|nil, child:entity, how?:string|boolean)
function lot(parent, child, how)
```

Groups an entity onto another. From then on the child's position, rotation and scale are relative to its parent: move, turn or scale the parent and the child goes with it, and the child's own numbers stay what you set. The engine works the tree out every frame, so set the child's local transform once and leave it. Order doesn't matter (a child can be made or grouped before its parent), a loop of parents is cut rather than followed, and it works the same on the web.

- `lot(parent, child)` rides the parent entity.
- `lot(parent, child, "bone")` rides one joint of the parent's skeleton (see [entity](#entity), **Skeletons**). The child's origin is the joint, and its axes are the model's own as it stands in its rest pose, so you place a hat on a head the way the two were modelled, not by working out which way the bone points. It then follows the clip, bone overrides and `ik` exactly, as a vertex weighted to that joint would. An unknown bone rides the entity.
- `lot(parent, child, true)` wears the parent's pose: a skinned child is drawn with the parent's joints, matched by name, instead of animating itself. Clothes cut from a body's rig play the body's clips, overrides and `ik` with no calls of their own. Joints the parent lacks keep their rest pose.
- `lot(nil, child)` lets go. The child's numbers are world numbers again, so set them in the same frame.

Children can have children (a face on a head on a body). Killing a parent doesn't kill its children: they fall back to their own numbers, so kill them too. Collision (`hit`) and `ent:bone` read an entity's own numbers and don't follow parents.

```lua
body = make("body", 10, 4, 0)
body:anim("walk")

shirt = make("shirt", 0, 0, 0)
lot(body, shirt, true)        -- skinned with the body's pose

head = make("head", 0, 0, 0)
lot(body, head, "head")       -- nods and tilts with the head bone
hat = make("hat", 0, 0, 0.3)
lot(head, hat)                -- 0.3 above the head, wherever that is

sword = make("sword", 0, 0, 0)
lot(body, sword, "hand.r")    -- carried
-- later: drop it where the character stands
lot(nil, sword)
sword:pos(body.x, body.y, body.z)
```

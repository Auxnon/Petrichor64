## app.newtex

_a fresh image for one entity_

```lua
---@type fun(ent:integer, name:string, w:integer, h:integer, mode:string?):{ok:boolean, tex:string?, path:string?, unwrapped:boolean?, charts:integer?, err:string?}
function app.newtex(ent, name, w, h, mode)
```

Gives entity `ent` (the `id` from `app.pick`) a new `w` x `h` texture called
`name`, painted to look the way the entity does now, so painting starts from
what was there.

- A mesh (from `mod()` or a glb) is unwrapped onto the image (see `unwrap`;
  `mode` is `"box"`, the default, or `"flat"`) and becomes a new model, also
  called `name`, which this entity switches to. Other entities sharing the old
  model are left alone. The first save writes `assets/name.glb`: the mesh with
  its new uvs and the image, since the app's code still describes the old ones.
  Set `asset = "name"` in the app to keep it.
- A sprite, or a model drawn with `tex`, keeps its mapping and just shows the
  new image. The first save writes `assets/name.png`.

Names are `a-z`, `0-9` and `_`, and must not be a texture, model or file the
app already has. A save never overwrites a file that appeared meanwhile.

Only exists inside an overlay.

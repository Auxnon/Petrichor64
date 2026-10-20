# Building games and apps on Petrichor64 — notes for agents

`CLAUDE.md` is about working *on* the engine. This file is about building a game
*with* it: what to write, the Lua traps that cost the most time, and how to test a
game without a human at the keyboard. It comes out of building
[Roomer](https://github.com/Auxnon/roomer) (a cozy 3D apartment: ~20 scripts, all
models and textures generated in code). Read Roomer's `scripts/` for working
examples of everything below.

## The 60-second version

1. A game is a folder: `scripts/*.lua` plus a **non-empty** `assets/` folder (drop
   an `icon.png` in it). Run it with `chor path/to/game` (the old
   `target/release/petrichor64` binary does the same). Build with
   `--features audio` if you want sound.
2. Define `main()` (runs once after load) and `loop()` (every frame). `draw(w, h)`
   is optional and runs on window resize, not every frame. Draw the HUD from
   `loop()`. (Before this was fixed, engines older than this commit fail every
   frame with `Value nil is not callable@1:1` if `draw` is missing, so define an
   empty one if you target them.)
3. **Z is up.** x/y are the ground plane.
4. silt (the Lua VM) miscompiles some ordinary Lua. Write in the restricted style
   below from the first line. Retrofitting it later is the slow part.
5. Test with a `TEST_HOOK(dt)` script on a hidden monitor (see *Testing* below).
   Don't wait for a human to click around.

## Write Lua in this style (silt workarounds)

Full details and repros are in `SILT-BUGS.md`. These rules avoid all of the open bugs.
Follow them everywhere, not only where something already broke. The failures show up
far from their cause (`Value: 0.76… is not callable`, `Invalid chunk due compilation
corruption`, or a whole file silently not loading).

- **Hoist every call out of argument lists and constructors.**
  `local c = cos(a)` then `f(c * r, x)`, never `f(cos(a) * r, x)`. This covers
  #1 and #14. It applies to string concatenation with calls too:
  `local name = "crop" .. tostring(s)`.
- **Table constructors that mention locals get their own statement:**
  `local p = { x, y, z }` then `arr[n] = p`. Never `arr[#arr + 1] = { x, y }` (#5).
- **No anonymous functions**, not even `pcall(function() … end)` (#10). Use named
  global functions and dispatch on strings or tables of names.
- **Don't write one-line function bodies that declare a local.** Split them over lines.
- **Never delete from tables.** `table.remove`/`table.insert(t, pos, v)`, `t[#t] = nil`
  and `t[k] = nil` all misbehave (#2, #3, #8, #9). Keep a count (`N = N + 1; T[N] = v`),
  flag dead entries (`it.dead = true`), or rebuild the array with a filter loop.
  Store sets as `true`/`false` and test with `== true`.
- **Entities in arrays:** `local e = ents[i]; e.z = 0` works. `ents[i].z = 0` errors (#11).
- **Float keys are not int keys.** `t[6/2]` is not `t[3]`. Use `flr()` (it returns an
  int) or `//` for anything used as a key or grid index.
- No `1e30` literals. vec3 fields are read-only. `break` works.
- Keep files small and split by topic. Each chunk has limits on constants and
  top-level globals (#4, #6). A dozen 150–400 line files is a comfortable size.
  Put shared state in a few global tables (`P`, `G`, `UI`, ...), not hundreds of
  loose globals.
- Scripts load in sorted filename order. Keep top level to definitions and
  constants, and do all setup inside `main()`, so load order never matters.

## World, camera, meshes

- `cam{pos={x,y,z}, rot={yaw, pitch}}`. Yaw `tau/4` looks toward +y. A negative
  pitch looks down. A simple follow camera: aim at the player, put the eye at
  `y - cos(pitch) * dist`, `z - sin(pitch) * dist`, and ease every value with
  `v = v + (target - v) * 0.08`.
- **Generate models in code with `mod()`.** The polygon form (`p` points, `f`
  1-based faces, `fu` one uv per face, `scale`) is fast and precise. Vertices pack
  at 1/16 unit, so build at `scale = 4` (or more) and shrink the entity with
  `e.scale = 1 / 4`. Roomer's `scripts/mesh.lua` is a reusable builder you can
  copy as-is, with box, cylinder, blob, disc and quad helpers, quarter-turn rotations,
  and `finish/place/spawn`.
- **One palette texture for everything.** Paint a small grid of flat colours into
  one texture (`scripts/pal.lua`) and give each face a uv pointing at one swatch.
  Every model shares it, so there's no texture juggling and recolours are one-line edits.
- For textured floors, the `v`/`i`/`u`/`n` form tiled cleanly. The `q` form left
  hairline gaps. Avoid coplanar faces (rugs on floors, sills in walls). Lift them
  a few hundredths, or they z-fight.
- **Billboard sprites** (`make(name, x, y, z, scale)` on a texture with alpha)
  work well for particles, icons and pointers. Changing `e.asset = "other"` swaps
  the look in place (sleeping cat, TV channels).
- **Don't create and kill entities per frame.** Make a fixed pool at startup and
  park unused ones far away (`e.z = -100`). `scripts/fx.lua` is a 40-sprite
  particle pool built this way.
- **Fog distance is in internal units, about 16× world units.** `fog(2600)`
  gives roughly 160 tiles. A small world needs much bigger numbers than you'd guess.
- The sky is a raster you can paint (gradients, stars, a moon). Roomer's
  `scripts/env.lua` does day and night.

## Skinned, animated glTF characters

The engine plays real skeletal animation from glTF, so characters don't have to be
rigid parts or sprite flipbooks. Full API: `guide/entity.md`, **Skeletons**.

- Drop a skinned `.glb` into `assets/`; it loads as a model named after the file.
  `make("farmer", x, y, z)`, then `e:anim("walk")` plays the clip of that name
  (names are lowercased). Clips loop. `e:anim("walk", true, 1.5)` restarts it at
  1.5× speed, and `e:anim("")` goes back to the rest pose. For a one-shot like a
  death, time it in your script and then switch to a held clip (`die`, then `dead`).
- `e.bones` lists the joints. `e:bone("hand.r")` reads a bone's world position, so
  you can attach a sprite to it. `e:bone("head", {rx, ry, rz})` adds a rotation on
  top of the clip, which is enough for things like aiming a head at the player.
- **It's true smooth skinning**: up to 4 joints and weights per vertex
  (`JOINTS_0`/`WEIGHTS_0`), up to 64 joints, with inverse bind matrices.
  Use it. If every vertex has weight 1 on a single bone, the model is a stack of
  rigid boxes and looks like Minecraft. Instead, make each arm one tube from
  shoulder to wrist and each leg one tube from hip to ankle, and give the rings
  either side of the elbow or knee a 50/50 or 85/15 split between the two bones.
  Do the same at the waist (hips/spine) and the neck (spine/head). Then joints
  crease instead of scissoring apart. Keep props like weapons rigid.
- What the loader needs: an **indexed** mesh, the texture as the **first embedded
  image** (nearest filtering suits the look), and **Z-up** vertex data. In Blender,
  untick "+Y Up" when exporting. Vertices pack at 1/16 unit, so author the model
  at 4× and show it with `e.scale = 0.25`.
- You can generate them in code with no Blender at all. Barrow
  (`~/projects/dark`, `tools/gen_assets.py`) is a dependency-free Python
  generator you can copy. It has a rig builder, `tube()` lofts with blended ring
  weights, a palette texture with hand-drawn face pictures, pose/blend helpers,
  and a glTF writer that bakes LINEAR quaternion keys plus a root translation per
  clip. The result is about 850 triangles per character with idle, walk, run,
  attack, hit and die clips.
- Facing: `e.rz = atan2(dy, dx)` turns an entity toward a target. Use the native
  `atan2(y, x)`; older engines lacked it, which is why old games carry a Lua one.

## Picking and input

- `mus()` returns a table: `x, y` (0–1 across the screen), `m1`/`m2`/`m3`
  booleans, `scroll`, and a
  world-space ray direction `vx, vy, vz`. The ray starts at the camera position
  you gave `cam()`, so keep your own copy of that position. Pick by
  ray-vs-AABB against your interactables (`W.ray_box` in `scripts/world.lua`).
  For a floor click, intersect with `z = 0`.
- Detect a click as an edge: `click = m.m1 and not last_m1`.
- **Route every input through plain functions** (`click_item(it)`,
  `player_go(x, y)`, `interact()`). Key handlers, mouse handlers and tests then
  all call the same code, which is what makes scripted testing possible.
- Also read the mouse through an override: `local m = mus(); if MOUSE_FAKE ~= nil
  then m = MOUSE_FAKE end`. Tests can then feed computed rays into the real picking code.

## GUI

- The `gui` layer is about **390×240** pixels. Integer coordinates are pixels and
  float coordinates are percentages. The font is caps-only with an 8px advance, so
  width = `#s * 8`. Write UI strings in caps.
- Use `rrect` with a translucent dark shadow offset 2–3px behind a light panel.
  It reads well over any 3D scene. Clamp tooltips to the screen edges.

## Grid pathfinding that works

Roomer's `scripts/world.lua` has a reusable version. Describe obstacles as AABB
footprints. Rasterize them onto a **0.25-unit** grid, padded by the walker's
radius (a 0.5 grid was too coarse for doorways). BFS from start to goal, snap
blocked goals to the nearest free cell, then **straighten** the path by dropping
waypoints while the straight segment stays clear. Doors are blockers that paths
can open: return them alongside the path and open each one as the walker
approaches.

## Testing without a human

The loop that worked: write a scripted playtest, run it on an invisible monitor,
read the log, look at screenshots, fix, repeat.

### 1. The test hook

In `loop(dt)`, add:

```lua
if TEST_HOOK ~= nil then
	TEST_HOOK(dt)
end
```

Each test is a file copied into a scratch copy of the game as
`scripts/zz_test.lua`. The name sorts last, so it loads after everything else. It
defines `TEST_HOOK` as a little timed step machine that calls the same functions
the input handlers call, and `print`s state at each step:

```lua
TT = { t = 0, step = 0 }
function TEST_HOOK(dt)
	TT.t = TT.t + dt
	if TT.step == 0 and TT.t > 1 then
		TT.step = 1
		click_item(STOVE)
	elseif TT.step == 1 and TT.t > 8 then
		TT.step = 2
		print("tea", TEA.state, P.x, P.y)
	end
end
```

Roomer's `playtests/` has working examples. `soak.lua` clicks random items and
floor spots for 70s, which is the cheapest way to catch rare Lua errors. Its
`run.sh` does the copy-and-launch.

### 2. Run it where nobody sees it (Hyprland)

Keyboard events from `hyprctl` don't reach an unfocused window, and moving the
real cursor disrupts the user. That's why tests drive game functions rather than
fake input. Put the window on a headless output:

```sh
hyprctl output create headless TEST
hyprctl eval 'hl.monitor({output="TEST", mode="1280x800@60", position="-4000x0", scale=1})'
hyprctl monitors | grep -A2 TEST         # note which workspace it got (it was 11 for us)
hyprctl eval 'hl.exec_cmd("[workspace 11 silent] /path/to/launch.sh /path/to/gamecopy")'
sleep 6; grim -o TEST shot.png           # screenshot just that output; then Read the png
pkill -x chor                            # stop the game
hyprctl output remove TEST               # clean up when done
```

This machine's Hyprland uses a **Lua config**, so configure it through
`hyprctl eval 'hl.…'`. Have `launch.sh` redirect the engine's stdout and
stderr to a log file. Your `print`s, the engine's `fps::` lines and Lua errors
all land there:

```sh
grep -E 'rror|not callable|corrupt|panic|Invalid|attempt' game.log
grep fps game.log | tail -3
```

If the session is **locked** (or there's no compositor at all), `grim` only
captures the lock screen. Have the engine save its own frames instead:

```sh
env CHOR_SNAP=/tmp/shot CHOR_SNAP_AT=200,400 chor path/to/game
# -> /tmp/shot-200.png, /tmp/shot-400.png (frame counts since start)
```

### 3. Gotchas while testing

- **Kill by exact name** (`pkill -x chor`). A `pkill -f` pattern that matches your
  own shell's command line kills the shell itself (exit 144).
- Wrap `grim` in `timeout 5`. It can hang if the output went away.
- Take screenshots at several times in one run (`sleep 6; grim …; sleep 6; grim …`)
  rather than relaunching for each. Startup takes about 2–3s.
- Hide welcome cards and tutorial overlays from the hook (`UI.welcome = 0`) so
  screenshots show the scene.
- Make each test step print the state you're asserting on (`TEA.state`,
  positions, counts). Reading the log is faster and more reliable than judging a
  screenshot.
- What this can't cover: real keyboard focus and real mouse events. Tell the user
  those still need a hands-on check.
- Entity `scale` is write-only: reading `e.scale` errors with "UserData field
  getter does not exist". Keep your own copy if you need it.

### 4. Testing an overlay

Overlays (editors that run over an app) load with
`chor path/to/game --overlay path/to/overlay`. Give the overlay's `loop` the
same `TEST_HOOK` call and drop a `zz_test.lua` into a scratch copy of it.
Overlay code can aim the picker at fixed screen points, so a test never needs
the real mouse: `app.pick(false, 0.5, 0.6)` returns what's under that point in
the app, and `app.pick(true, x, y)` hits the overlay's own 3D panels (with
`app.project(x, y, z)` to find where they are on screen).

## Overlays: editing a running app

**Ctrl+P** opens the built-in paint overlay (`apps/paint3d`) over any app that
hasn't set `attr{lock=true}`. It pauses the app and paints textures straight
onto the meshes. It's also the worked example for writing your own overlay.
Overlays get a privileged `app.*` table (guide: `pick.md`, `pause.md`,
`view.md`, `colors.md`, `project.md`, `close.md`, `source.md`, `save.md`,
`info.md`, `texinfo.md`, `newtex.md`):

- `app.pick()` ray-casts what's actually drawn (model triangles, billboard
  sprites, tiles) and reports the texture and texel under the pointer. Edit
  that texture with `gimg`/`tex` and every mesh using it updates live.
- An overlay that never calls `cam` draws through the app's camera. Set
  `attr{front=true}` so its entities draw over the scene instead of inside it,
  and `e.unlit = true` so the app's lights and fog don't shade your UI.
- Build a 3D panel as one image on a billboard (`make(texname, ...)`). The
  texel `app.pick(true)` returns *is* the panel pixel, so button hit-testing is
  plain 2D maths in image space.
- `app.save(texture)` writes an edited texture back into the image file the
  app loaded it from (a tile only into its square of the sheet). Only the
  edited app's own png files qualify: a texture the app drew from code
  (`nimg` + `tex`) has no file, and `app.source` says so before you ask. The
  paint overlay's SAVE button (lower right, once something's painted) does
  this behind a confirm dialog. A packed game (`app.info().packed`) has no
  files: every save refuses, and the overlay shows READ ONLY.
- `app.newtex(ent, name, w, h, mode)` gives one entity a fresh image, painted
  to look the way it does now. A mesh is unwrapped onto it and becomes a new
  model `name` that the entity switches to; the first save writes
  `assets/name.glb` (mesh, new uvs, image). A sprite gets `assets/name.png`.
  The paint overlay's NEW button opens a modal for this.
- `unwrap(mesh, {w, h, pad, mode})` (any app, not just overlays) works out uvs
  for a mesh: pass its `v`/`n`/`u`/`i` straight to `mod()`.

## Suggested game layout

```
scripts/main.lua      main/loop/draw, camera, input → calls into the rest
scripts/pal.lua       palette texture + named colour uvs
scripts/mesh.lua      mesh builder
scripts/tex.lua       generated textures and sprites
scripts/world.lua     blockers, pathfinding, interactable items, ray picking
scripts/player.lua    movement and poses
scripts/play.lua      what each interaction does (state machines)
scripts/ui.lua        HUD, prompts, toasts
scripts/fx.lua        particle pool
scripts/sfx.lua       instruments and sound helpers
playtests/*.lua       TEST_HOOK scripts + run.sh
```

Every interactable is one record: name, AABB, reach and a `kind` string that
`play.lua` dispatches on. Hover labels, the E-key target, click
picking and tests then all read the same list.

## UNRELEASED

## 0.4.0

- New `conn` command to make a tcp connection to an ip or address! Very early version only works with sending strings with a carriage return indiciating termination. Should work with a ping pong server if you check for '\r'. More to come!
- Headless binary now available for server like setup without a render pipeline. Just have an app run on init
- In accordance with shorthand command names: `spawn` is now `make`, `mouse` is now `mus`, `button` is now `btn`, `analog` is now `abtn`, `sound` is now `note`, `silence` is now `mute`, `model` is now `mod`, `lmodel` is now `lmod`, `group` is now `lot`. `subload` is now simply `sub` (Would someone mistake a subprocess command as subtract? Time will tell. ), `overload` is now `over`, `input` is now `cin`, `dchunk` and `dtiles` are now merged into a single method `dtile`.
- `log` which acts like `print` which it also overrides, is now called by `cout` so no one is confused thinking it's logarithm. Is it worth saving a single character? Yes. `print` still works too.
- `print`/`cout` now works with variable number of arguments (including images and entities) as it normally should without having to concat first. `print('test',1,2)` yields _test, 1, 2_
- Added `pow` for exponents, `log` is now for base 10 logarithms as you'd expect
- `rou` to round a value instead of just floor or ceiling
- New notification system as a direct overlay for spammed errors. More usage coming soon!
- Much smaller error output, hopeully easier to read in console or notifications
- New system font!
- Testing a cross platform error popup, should work even if render pipeline fails.
- Text can now be colored!
- `gtile` command to get asset/texture at tile position, use for checking what's placed where
- Disabled "smooth" camera movement as it was ironically shakey
- Added bad quad data failsafes
- Models with quads can have optional uvs and indexes changed directly just like triangle based models. 4 UV coords per quad. Indexes still optional.
- Initial work for lighting systems, able to include normals in model data
- Mouse command has scroll delta `mus().scroll`
- Img raster can use pixel command to set individual pixels, not super efficient
- Bug prevented sky raster from updating at the same time as the main raster
- Major change to how image rasters are layered on to the draw target. Now uses the GPU for hella quick overlaying
- Adding a `draw` function to your app or game will be called everytime the the windows resizes. Builtin debounce. This is useful if you don't desire to constantly redraw every loop
- Clearing sky raster actually works now too
- Corrected precision error with some math functions using 32 bit floats instead of 64 bit
- Camera now defaults to 0 azimuth like you'd expect
- `tex` function is now synchronous to prevent unexpected texturing behavior
- new example app is a little more fleshed out
- Codex 3.0

## 0.3.0

- First big update!
- Sound is on again, oops. Sound libraries were literally not included in previous versions. 🤦‍♂️
- Building inline models with quad arrays will smart unwrap provided texture assets to align to the first 2 vectors of a quad. A skewed quad will still align to this first segment and simply cut into the texture. It will assume aspect ratio for UVs coordinates from the larger of the xy axis to be 1 and the smaller a ratio of their lengths. It'll make more sense if you see it action. Inline models can also have UVs sent in directly if you're hardcore about it or not getting what you want.
- Changed entity.tex to a smart field, not a function, to match up with other properties of an entity. Entities already smartly update the engine with their properties, so encapsulating a texture change into a function was unnecessary. Just say `entity.tex="new texture"`. Easy!
- entity.asset similarly allows changing the model of an entity, if none are found it will default to a blocked version of a texture of the same name ( assuming there is one ). This behavior matches how assets are provided in the spawn function. Keep in mind models will apply their own stored texture mapping when this is used, so if the texture is intended to be overriden you will want to set entity.tex to that new texture after setting the asset
- Codex updated with image userdata type information, all custom types are lowercase to match the aesthetic
- Any codex change like this that's not backwards compatible, no matter how small, means a version bump. Codex is now at 2.0.0 "avacado". The Codex naming is just for dev users to track more easily

## 0.2.4

- removed some unnecessary console printing
- ent offset for better rotation
- some codex bad params
- codex lsp diagnostics to allow lowercase globals
- auto game loader works with new console app

## 0.2.1 - 0.2.3

- various patches to get realease working with new test cases confirming or denying working status
- new render cycle immensely more performant then previous iterations, decoupled user input from renderer ( like it should have been )
- still forming a better system for distributing updates for each OS, initial local CI scripts

## 0.2

- first standalone release

## 0.1

- Only released bundled in the Potion Commotion game

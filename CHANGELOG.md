## UNRELEASED

## 0.3.0

- Sound is on again, oops. Sound libraries were literally not included in previous versions. 🤦‍♂️
- changed entity.tex to a smart field, not a function, to match up with other properties of an entity. Entities already smartly update the engine with their properties, so encapsulating a texture change into a function was unnecessary. Just say entity.tex="new texture". Easy!
- entity.asset similarly allows changing the model of an entity, if none are found it will default to a blocked version of a texture of the same name ( assuming there is one ). This behavior matches how assets are provided in the spawn function. Keep in mind models will apply their own texture mapping when this is used, so if the texture is intended to be overriden you will want to set entity.tex again that new texture after setting this
- Codex updated with image userdata type information, all custom types are lowercase to match the aesthetic
- Any codex change like this that's not backwards compatible, no matter how small, means a version bump. Codex is now at 2.0.0 "avacado"

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

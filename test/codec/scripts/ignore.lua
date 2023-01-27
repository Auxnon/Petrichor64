-- Codex 1.0.0 "Applesauce"
-- this file is ignored at runtime, we can use this to define commands for a LSP
-- Feel free to add to this file knowing it will not be bundled with the game or application

---@class Mouse
---@field x number
---@field y number
---@field dx number delta x
---@field dy number delta y
---@field m1 boolean mouse 1
---@field m2 boolean mouse 2
---@field m3 boolean mouse 3
---@field vx number unprojection x
---@field vy number unprojection y
---@field vz number unprojection z
---@return Mouse

---@class Attributes
---@field resolution number artificial resolution
---@field lock boolean
---@field fog number 0 is off
---@field fullscreen boolean
---@field mouse_grab boolean
---@field size integer[] width, height of window
---@field title string
---@field modernize boolean must be false or 0 for the remainder to work
---@field dark number
---@field glitch number[]
---@field curvature number
---@field flatness number
---@field high number
---@field low number
---@field bleed number

---@class CamParams
---@field pos number[]? x, y, z
---@field rot number[]? azimuth, altitude

--- @class ModelData
--- @field t string[]? texture assets
--- @field q number[][]? quads
--- @field v number[][]? vertices
--- @field u number[][]? uvs
--- @field i number[][]? indicies


--- @class Entity
--- @field x number x position
--- @field y number y position
--- @field z number z position
--- @field rx number rotation x
--- @field ry number rotation y
--- @field rz number rotation z
--- @field vx number velocity x
--- @field vy number velocity y
--- @field vz number velocity z
--- @field flipped number texture flip x axis
--- @field scale number uniform scale factor 1 is 100%
--- @field id integer assigned by engine, killable
--- @field tex function string asset
--- @field anim function string animation
--- @field kill function destroy entity

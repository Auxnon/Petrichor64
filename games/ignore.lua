-- Codex 1.0.0 "Applesauce"
---@diagnostic disable: duplicate-doc-field, missing-return
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

-- Check if a tile is present at a given location
---@param x integer
---@param y integer
---@param z integer
---@return boolean
function istile(x, y, z) end

-- Spawn an entity from an asset
---@param asset string
---@param x number
---@param y number
---@param z number
---@param scale number?
---@return Entity
function spawn(asset, x, y, z, scale) end

-- Stop sounds on channel
---@param channel number
function silence(channel) end

-- Clear the draw target
function clr() end

-- Draw a line on the draw target
---@param x number
---@param y number
---@param x2 number
---@param y2 number
---@param rgb number[]? | string? rgba number array or hex string
function line(x, y, x2, y2, rgb) end

-- Check if key is held down
---@param key string
---@param volatile boolean? only true on first frame of key press
---@return boolean
function key(key, volatile) end

-- Sets image data as a texture
---@param asset string
---@param im userdata
function tex(asset, im) end

-- Draw image on the gui at position
---@param im userdata
---@param x number?
---@param y number?
function img(im, x, y) end

-- Absolute value
---@param f number
---@return number
function abs(f) end

-- Set the camera position and/or rotation
---@param params CamParams
function cam(params) end

-- Hard quit or exit to console
---@param u integer? >0 soft quits
function quit(u) end

-- Set various app state parameters
---@param attributes Attributes
function attr(attributes) end

-- insert model data into an assett <name:string, {v=[float,float,float][],i=int[],u=[float,float][]}>
---@param asset string
---@param t ModelData
function model(asset, t) end

-- Set background color of raster
---@param rgb number[] | string rgba number array or hex string
function fill(rgb) end

-- Set front screen (gui) as draw target
function gui() end

-- Draw text on the gui at position
---@param txt string
---@param x number
---@param y number
---@param rgb number[]? | string? rgba number array or hex string
---@param typeset string? font name or size
function text(txt, x, y, rgb, typeset) end

-- Send UDP
-- Coming soon


-- Set an animation by passing in series of textures
---@param name string
---@param items string[]
---@param speed number?
function anim(name, items, speed) end

-- Removes an entity
---@param ent Entity | integer
function kill(ent) end

-- Prints string to console
---@param message string
function log(message) end

-- Remove all tiles from the world
function dtiles() end

-- load an overlaying bundle
---@param str string
function overload(str) end

-- List models by search
---@param model string
---@param bundle integer?
---@return string[]
function lmodel(model, bundle) end

-- Check if gamepad button is held down
---@param button string
---@return boolean
function button(button) end

-- Make sound
---@param freq number
---@param length number?
function sound(freq, length) end

-- Make a song
---@param notes number[][] | number[] nested array first is frequency, second is length
function song(notes) end

-- Get image buffer userdata for editing or drawing
---@param asset string
---@return userdata
function gimg(asset) end

-- Squareroot value
---@param f number
---@return number
function sqrt(f) end

-- Sine value
---@param f number
---@return number
function sin(f) end

-- Ceil value
---@param f number
---@return integer
function ceil(f) end

-- Cosine value
---@param f number
---@return number
function cos(f) end

-- Random float from 0-1, or provide a range
---@param a number?
---@param b number?
---@return number
function rnd(a, b) end

-- Set skybox as draw target
function sky() end

-- Floor value
---@param f number
---@return integer
function flr(f) end

-- An imperfect random number generator for integers. May suffer from modulo bias, only i32
---@param a integer?
---@param b integer?
---@return integer
function irnd(a, b) end

-- Load a sub bundle
---@param str string
function subload(str) end

-- Create new image buffer userdata, does not set as asset
---@param w integer
---@param h integer
---@return userdata
function nimg(w, h) end

-- Set color of pixel at x,y
---@param x integer
---@param y integer
---@param rgb number[] | string rgba number array or hex string
function pixel(x, y, rgb) end

-- Reset lua context
function reload() end

-- Groups an entity onto another entity
---@param parent Entity
---@param child Entity
function group(parent, child) end

-- Get a string of all keys pressed
---@return string
function input() end

-- Check how much a gamepad is pressed, axis gives value between -1 and 1
---@param button string
---@return number
function analog(button) end

-- Make an instrument
---@param freqs number[]
---@param half boolean? subsequent freqs are half the previous
function instr(freqs, half) end

-- Recieve UDP
-- Coming soon


-- Draw a rounded rectangle on the draw target
---@param x number
---@param y number
---@param w number
---@param h number
---@param ro number radius of corners
---@param rgb number[]? | string? rgba number array or hex string
function rrect(x, y, w, h, ro, rgb) end

-- Get mouse position, delta, button states, and unprojected vector
---@return Mouse
function mouse() end

-- Draw a rectangle on the draw target
---@param x number
---@param y number
---@param w number
---@param h number
---@param rgb number[]? | string? rgba number array or hex string
function rect(x, y, w, h, rgb) end

-- Crude deletion of a 16x16x16 chunk. Extremely efficient for large area tile changes
---@param x integer
---@param y integer
---@param z integer
function dchunk(x, y, z) end

-- Set a tile within 3d space. Nil asset deletes.
---@param asset string
---@param x integer
---@param y integer
---@param z integer
---@param rot integer?
function tile(asset, x, y, z, rot) end

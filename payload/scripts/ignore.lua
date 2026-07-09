-- Codex 3.0.0 "Artichoke"
---@diagnostic disable: duplicate-doc-field, missing-return
---@meta

---@class mouse
---@field x number
---@field y number
---@field dx number delta x
---@field dy number delta y
---@field m1 boolean mouse 1
---@field m2 boolean mouse 2
---@field m3 boolean mouse 3
-- @field scroll number scroll delta
---@field vx number unprojection x
---@field vy number unprojection y
---@field vz number unprojection z
---@return mouse

---@class attributes
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

---@class cam_params
---@field pos number[]? x, y, z
---@field rot number[]? azimuth, altitude

--- @class model_data
--- @field t string[]? texture assets
--- @field q number[][]? quads
--- @field v number[][]? vertices
--- @field u number[][]? uvs
--- @field i integer[]? indices

--- @class entity
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
--- @field offset number[]? x, y, z position to offset model
--- @field id integer assigned by engine, used for destroying
--- @field tex string texture asset
--- @field asset string model or blocked texture asset
--- @field anim fun(self:entity,animation:string,force?:boolean) change animation, force marks change even if already playing
--- @field kill fun(self:entity) destroy entity

--- @alias gunit number | integer | string
--- @alias rgb number[] | integer[] | string | integer

--- @class image
--- @field line fun(self:image, x:gunit, y:gunit, x2:gunit, y2:gunit, rgb?:rgb) draw line on image
--- @field rect fun(self:image, x:gunit, y:gunit, w:gunit, h:gunit, rgb?:rgb) draw rectangle on image
--- @field rrect fun(self:image ,x:gunit, y:gunit, w:gunit, h:gunit,ro:gunit, rgb?:rgb) draw rounded rectangle on image
--- @field text fun(self:image, txt:string, x?:gunit, y?:gunit, rgb?:rgb) draw text on image
--- @field img fun(self:image, im:image, x?:gunit, y?:gunit) draw another image on image
--- @field pixel fun(self:image, x:integer, y:integer,rgb?:rgb) draw pixel directly on image
--- @field clr fun(self:image) clear image
--- @field fill fun(self:image, rgb?:rgb) fill image with color
--- @field raw fun(self:image):integer[] image return raw pixel data
--- @field copy fun(self:image):image clones to new image

--- @class connection
--- @field send fun(self:connection, data:string) send data to connection
--- @field recv fun(self:connection):string | nil receive data from connection
--- @field test fun(self:connection):string | nil test if connection is still alive, returns string for error, 'safe close' for no error
--- @field kill fun(self:connection) close connection


--- @type number ~3.1457
pi = nil
--- @type number ~6.2914
tau = nil
--- @type image image raster for the front screen
gui = nil
--- @type image image raster for the back screen or 'sky'
sky = nil

--- shorthand for gui:text
function text(...) end

--- shorthand for gui:line
function line(...) end

--- shorthand for gui:rect
function rect(...) end

--- shorthand for gui:rrect
function rrect(...) end

--- shorthand for gui:img
function img(...) end

--- shorthand for gui:pixel
function pixel(...) end

--- shorthand for gui:fill
function fill(...) end

--- shorthand for gui:clr
function clr(...) end



-- nil
Set an animation by passing in series of textures


-- nil
Round value


-- nil
Set a tile within 3d space. Nil asset deletes.


-- nil
Absolute value


-- nil
Return the asset name of the tile at a given location


-- nil
Make a sound or note


-- nil
Make a song


-- nil
Squareroot value


-- nil
Load a sub bundle


-- nil
Floor value


-- nil
Squareroot value


-- nil
Crude deletion of a 16x16x16 chunk. Extremely efficient for large area tile changes. Not including arguments delete all tiles.


-- nil
Get image buffer userdata for editing or drawing


-- nil
Get mouse position, delta, button states, and unprojected vector


-- nil
Check if a tile is present at a given location


-- nil
Groups an entity onto another entity


-- nil
List models by search


-- nil
Stop sounds on channel


-- nil
Check how much a gamepad is pressed, axis gives value between -1 and 1


-- nil
Set app state params or get attributes if no args


-- nil
Get a string of all keys pressed


-- nil
An imperfect random number generator for integers. May suffer from modulo bias.


-- nil
Cosine value


-- nil
Removes an entity


-- nil
Random float from 0-1, or provide a range


-- nil
insert model data into an asset <name:string, {v=[float,float,float][],i=int[],u=[float,float][]}>


-- nil
Sine value


-- nil
Spawn an entity from an asset


-- nil
Ceil value


-- nil
Check if key is held down


-- nil
Base 10 logarithm of the value


-- nil
Create a new connection to the ip, site, etc. A :port is optional


-- nil
Prints string to console


-- nil
Make an instrument


-- nil
Hard quit or exit to console


-- nil
Find first occurence of a tile in a given direction


-- nil
Sets image data as a texture


-- nil
Check if gamepad button is held down


-- nil
Create new image buffer userdata, does not set as asset


-- nil
load an overlaying bundle


-- nil
nil


-- nil
Resets lua context and reloads all assets and scripts fresh


-- nil
Set the camera position and/or rotation



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
--- @field i integer[]? indicies

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
--- @field offset number[]? x, y, z
--- @field id integer assigned by engine, killable
--- @field tex string texture asset
--- @field asset string model or blocked texture asset
--- @field anim fun(self:entity,animation:string,force?:boolean) change animation, force marks change even if already playing
--- @field kill fun(self:entity) destroy entity

--- @alias gunit number | integer | string
--- @alias rgb number[] | integer[] | string

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
function text(...)
end

--- shorthand for gui:line
function line(...)
end

--- shorthand for gui:rect
function rect(...)
end

--- shorthand for gui:rrect
function rrect(...)
end

--- shorthand for gui:img
function img(...)
end

--- shorthand for gui:pixel
function pixel(...)
end

--- shorthand for gui:fill
function fill(...)
end

--- shorthand for gui:clr
function clr(...)
end

-- Absolute value
---@param f number
---@return number
function abs(f)
end

-- insert model data into an asset <name:string, {v=[float,float,float][],i=int[],u=[float,float][]}>
---@param asset string
---@param t model_data
---@return string stating what mode the model was built in
function mod(asset, t)
end

-- Removes an entity
---@param ent entity | integer
function kill(ent)
end

-- Squareroot value
---@param f number
---@return number
function sqrt(f)
end

-- Load a sub bundle
---@param str string
function sub(str)
end

-- Set the camera position and/or rotation
---@param params cam_params
function cam(params)
end

-- Set a tile within 3d space. Nil asset deletes.
---@param asset string
---@param x integer
---@param y integer
---@param z integer
---@param rot integer?
function tile(asset, x, y, z, rot)
end

-- Floor value
---@param f number
---@return integer
function flr(f)
end

-- Check if gamepad button is held down
---@param button string
---@return boolean
function btn(button)
end

-- Crude deletion of a 16x16x16 chunk. Extremely efficient for large area tile changes. Not including arguments delete all tiles.
---@param x? integer
---@param y? integer
---@param z? integer
function dtile(x, y, z)
end

-- Check if key is held down
---@param key string
---@param volatile boolean? only true on first frame of key press
---@return boolean
function key(key, volatile)
end

-- Check how much a gamepad is pressed, axis gives value between -1 and 1
---@param button string
---@return number
function abtn(button)
end

-- Get mouse position, delta, button states, and unprojected vector
---@return mouse
function mus()
end

-- Get a string of all keys pressed
---@return string
function cin()
end

-- Create a new connection to the ip, site, etc. A :port is optional
--- @param addr string
--- @return connection
function conn(addr)
end

-- Prints string to console
---@param ... string|number|integer|boolean|image|entity|connection|nil
function cout(...)
end

-- Find first occurence of a tile in a given direction
---@param x integer
---@param y integer
---@param z integer
---@return string
function gtile(x, y, z)
end

-- Random float from 0-1, or provide a range
---@param a number?
---@param b number?
---@return number
function rnd(a, b)
end

-- Return the asset name of the tile at a given location
---@param x integer
---@param y integer
---@param z integer
---@return string
function gtile(x, y, z)
end

-- Groups an entity onto another entity
---@param parent entity
---@param child entity
function lot(parent, child)
end

-- Cosine value
---@param f number
---@return number
function cos(f)
end

-- Set an animation by passing in series of textures
---@param name string
---@param items string[]
---@param speed number?
function anim(name, items, speed)
end

-- Set various app state parameters
---@param attributes attributes?
function attr(attributes)
end

-- Sets image data as a texture
---@param asset string
---@param im image
function tex(asset, im)
end

-- Check if a tile is present at a given location
---@param x integer
---@param y integer
---@param z integer
---@return boolean
function istile(x, y, z)
end

-- Squareroot value
---@param f number target
---@param e number raise by
---@return number
function pow(f, e)
end

-- Ceil value
---@param f number
---@return integer
function ceil(f)
end

-- An imperfect random number generator for integers. May suffer from modulo bias, only i32
---@param a integer?
---@param b integer?
---@return integer
function irnd(a, b)
end

-- Make an instrument
---@param freqs number[]
---@param half boolean? subsequent freqs are half the previous
function instr(freqs, half)
end

-- Resets lua context and reloads all assets and scripts fresh
function reload()
end

-- load an overlaying bundle
---@param str string
function over(str)
end

-- Sine value
---@param f number
---@return number
function sin(f)
end

-- Stop sounds on channel
---@param channel number
function mute(channel)
end

-- List models by search
---@param model string
---@param bundle integer?
---@return string[]
function lmod(model, bundle)
end

-- Create new image buffer userdata, does not set as asset
---@param w integer
---@param h integer
---@return image
function nimg(w, h)
end

-- Make a sound or note
---@param freq number
---@param length number?
function sound(freq, length)
end

-- Base 10 logarithm of the value
---@param f number target
---@return number
function log(f)
end

-- Spawn an entity from an asset
---@param asset string
---@param x number
---@param y number
---@param z number
---@param scale number?
---@return entity
function make(asset, x, y, z, scale)
end

-- Make a song
---@param notes number[][] | number[] nested array first is frequency, second is length
function song(notes)
end

-- Get image buffer userdata for editing or drawing
---@param asset string
---@return image
function gimg(asset)
end

-- Hard quit or exit to console
---@param u integer? >0 soft quits
function quit(u)
end

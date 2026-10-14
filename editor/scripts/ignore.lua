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
--- @field t (string|string[])? texture asset(s)
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
Set app state params or get attributes if no args


-- nil
Resets lua context and reloads all assets and scripts fresh


-- nil
Set the transport tempo, shared by every channel


-- nil
Pairwise collision test between two entities: nil or {normal={x,y,z}, depth=n}


-- nil
Prints string to console


-- nil
Find first occurence of a tile in a given direction


-- nil
Check how much a gamepad is pressed, axis gives value between -1 and 1


-- nil
Squareroot value


-- nil
Define instrument `id`: a waveform name (square/saw/tri/pulse/noise/sine) or a harmonic-amplitude table; cfg = { wid, atk, dec, sus, rel }


-- nil
Get or set the display monitor preset: lcd (modern flat panel), slot (slot-mask CRT), grille (aperture-grille CRT)


-- nil
Set the camera position and/or rotation


-- nil
Cosine value


-- nil
Every hit between entity `id`'s collider and any other entity's tile grid: array of {owner=id, tile={x,y,z}, normal={x,y,z}, depth=n}


-- nil
Every solid tile entity `id`'s collider currently overlaps: array of {tile={x,y,z}, normal={x,y,z}, depth=n}


-- nil
An imperfect random number generator for integers. May suffer from modulo bias.


-- nil
Random float from 0-1, or provide a range


-- nil
load an overlaying bundle


-- nil
Set an animation by passing in series of textures


-- nil
Record a microphone snippet into a sample slot: mic(id, secs?). mic() = is a capture running?


-- nil
Round value


-- nil
Set the one light: shape ("sun"/"cone"/"sphere", default sun), dir/pos (xyz), color (rgb 0..1), ambient (0..1), sky/ground (hemisphere, sun only), range, angle (cone half-angle, radians)


-- nil
Get a string of all keys pressed


-- nil
Sets image data as a texture


-- nil
Play a note on the transport: at a beat, or quantized to a grid


-- nil
insert model data into an asset <name:string, {v=[float,float,float][],i=int[],u=[float,float][]}>


-- nil
Drive/distort a channel: (channel, amount 0..1, curve 'soft'|'hard'|'fold'). amount<=0 disables it


-- nil
List models by search


-- nil
nil


-- nil
Spawn an entity from an asset


-- nil
Sine value


-- nil
Add reverb to a channel: (channel, room/decay 0..1, damp 0..1, wet mix). room<=0 disables it


-- nil
Base 10 logarithm of the value


-- nil
Return the asset name of the tile at a given location


-- nil
Get mouse position, delta, button states, and unprojected vector


-- nil
Set a channel's singing voice character for its later sing() notes: (channel, { breath, vib, hz })


-- nil
Hard quit or exit to console


-- nil
Grab (capture) the mouse for relative look, or release it with mgrab(false)


-- nil
Check if gamepad button is held down


-- nil
Get image buffer userdata for editing or drawing


-- nil
Resonant filter on a channel: (channel, kind 'low'|'high'|'band'|'notch', cutoff Hz, q, sweep secs). kind off/nil disables


-- nil
Fade a channel's volume to target (0..1, default 0) over secs — fade in/out or crossfade channels


-- nil
Get or set the graphics chip preset: r00 (modern), r43 (N64-ish), r30 (PS1-ish)


-- nil
Bitcrush a channel: (channel, bits 1..16, rate Hz). bits<=0 disables it


-- nil
Enable Gouraud (per-vertex) shading, or disable it with gour(false) to go back to per-fragment


-- nil
Absolute value


-- nil
Load a sub bundle


-- nil
Ceil value


-- nil
Play a note; optional channel + instrument. Overlapping notes voice separately.


-- nil
Add a feedback-delay echo to a channel: (channel, secs, feedback 0..1, wet mix). secs<=0 disables it


-- nil
MIDI in: midi() drains events, midi('ports'|'port'), midi('open', name?), midi('close')


-- nil
Enable the shadow map (from the current lum light), or disable it with shdw(false)


-- nil
Create a new connection to the ip, site, etc. A :port is optional


-- nil
Sing a syllable or space-separated phrase over a melody (a pitch, or a table of pitches / {freq,len} pairs)


-- nil
Play several frequencies at once as a chord


-- nil
Create new image buffer userdata, does not set as asset


-- nil
Every currently-overlapping ent-vs-ent pair in this bundle: array of {a=id, b=id, normal={x,y,z}, depth=n}


-- nil
Removes an entity


-- nil
Squareroot value


-- nil
Make a song


-- nil
Floor value


-- nil
Stop sounds on a channel, or all channels if omitted


-- nil
Set a tile within 3d space. Nil asset deletes.


-- nil
Set distance fog: color (rgb 0..1) blended in by dist (far, world units; 0 = off)


-- nil
Define instrument `id` from a loaded sound name, or raw PCM samples (-1..1); cfg = { base, atk, dec, sus, rel }


-- nil
Check if key is held down


-- nil
Check if a tile is present at a given location


-- nil
Groups an entity onto another entity


-- nil
Crude deletion of a 16x16x16 chunk. Extremely efficient for large area tile changes. Not including arguments delete all tiles.



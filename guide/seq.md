## seq

_play a sequence of notes or samples on the transport, once, counted, or looped_

```lua
---@type fun(id?: integer, steps?: table[], cfg?: table): integer?
function seq(id, steps, cfg)
```

Hands a whole pattern to the audio thread, which plays it against the sample
clock (see `bpm`, `cue`). Every pass lands exactly on the beat, so a looped drum
break and the bass under it never drift apart, and the game loop doesn't have to
do anything while it plays.

- `id` — your handle for the sequence. Calling `seq` again with the same `id`
  swaps in the new pattern **at the end of the current pass**, so a change never
  lands mid-bar. `seq(id)` stops it; `seq()` stops every sequence. Stopping lets
  notes already sounding ring out (use `mute(ch)` to cut them).
  `nil` with steps **pushes**: the sequence gets a new id and plays after the
  last sequence sent to its channel (with or without an id), or right away if
  that one is done. A push after a `loop = true` sequence never starts.
  `seq` returns the id, so a pushed sequence can be stopped or chained to.
- `steps` — played back to back. Each step is a table:
  - `[1]` beats until the next step (`0` stacks the next step on this one);
  - `[2]` frequency, a table of frequencies (a chord), or `0`/absent for a rest;
  - `[3]` instrument id (default `0`); `[4]` volume (default `1`);
  - `len=` how many beats it sounds (default: its beats; `0.25` for a `0`-beat step);
  - `at=` an explicit beat in the pattern, instead of following the previous step;
  - `say=` a syllable to **sing** at that pitch instead of playing the
    instrument (same spelling as `sing`), so lyrics ride the same clock.
    A syllable's opening s/t/k/sh... starts a little *before* its beat (up to
    60ms) so the vowel lands on it, and consecutive voiced syllables on a
    channel run together as one voice.
- `cfg` (all optional):
  - `ch` — channel (default `0`); effects and `vox` are the channel's;
  - `loop` — passes to play: `1` (default), `4`, or `true`/`0` for forever;
  - `len` — beats per pass (default: where the last step ends);
  - `at` — absolute transport beat to start on; `grid` — snap the start up to the
    next multiple of this many beats (`4` = next bar);
  - `after` — start when sequence `after` finishes, on its last beat (if it isn't
    playing, start now); `after = false` makes a push start now instead of queueing;
  - `bpm` — switch the tempo when this sequence starts, on its first beat. It
    stays after the sequence ends; sequences without `bpm` keep whatever tempo
    they find. A `bpm()` call still changes it at once, which is what live play
    wants;
  - `choke` — each step cuts what's still sounding on the channel (chopped breaks).

```lua
bpm(170)
-- a counted loop: a bass riff, 4 times, starting on the next bar
seq(1, { {1, 55}, {0.5, 0}, {0.5, 55}, {1, 65.4}, {1, 49} }, { ch = 1, loop = 4, grid = 4 })

-- chained: an intro once, then the main loop forever
seq(2, { {2, {261.6, 329.6, 392}}, {2, {220, 261.6, 329.6}} }, { ch = 2 })
seq(3, { {1, 392, 0, 1, say = 'la'}, {1, 440, 0, 1, say = 'di'} }, { ch = 3, after = 2, loop = true })

-- a break chopped into slices with smpl{from,to}, choked, forever
smpl(10, 'amen', { from = 0, to = 0.25 })
smpl(11, 'amen', { from = 0.25, to = 0.5 })
seq(4, { {1, 440, 10}, {1, 440, 11}, {0.5, 440, 10}, {0.5, 440, 10}, {1, 440, 11} }, { ch = 4, loop = true, choke = true })

seq(4)  -- stop the break
seq()   -- stop everything

-- a song as a queue on one channel: no ids, each plays after the last
local verse = { {1, 220}, {1, 247}, {2, 262} }
seq(nil, verse, { ch = 5, loop = 2 })
seq(nil, verse, { ch = 5, bpm = 180 })  -- the same pattern again, faster
local outro = seq(nil, { {4, 196} }, { ch = 5, bpm = 120 })
seq(nil, { {4, {98, 147}} }, { ch = 6, after = outro })  -- another channel can follow it too
```

Up to 32 sequences run at once. See also `bpm`, `cue`, `song`, `smpl`, `sing`.

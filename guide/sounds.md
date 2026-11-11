## sounds

_what's sounding right now: the transport, channel levels, sequences and voices_

```lua
---@type fun(t?: number[]): number[]
function sounds(t)
```

A snapshot of the audio thread, at most ~5ms old, as one flat array of numbers.
It's flat so reading it is just indexing: no strings to unpack, no nested tables
to allocate. Pass the last result back in (`S = sounds(S)`) and it's refilled in
place, so polling every frame makes no garbage.

| index | value |
|---|---|
| 1 | transport beat (fractional) |
| 2 | bpm |
| 3 | `nch`, channels |
| 4 | `nseq`, running sequences |
| 5 | `nvoice`, sounding voices |
| 6 … 5+nch | each channel's peak level since the last snapshot (after its effects and fade), channel 0 first |
| then | `nseq` records of 9: `id, ch, pass, loops, step, hits, phase, len, state` |
| then | `nvoice` records of 7: `ch, lane, instrument, freq, level, kind, releasing` |

A sequence record:
- `pass` — the pass it's in, from 0; `loops` its count (`0` = forever);
- `step` — the step last reached, from 1 (`0` before the first);
- `hits` — notes it has started (rests don't count). It goes up by one on every
  hit, so a display can **bump when it changes**;
- `phase` — beats into the current pass, of `len` beats per pass. Its
  fractional part is where it is in the beat: a pulse on every beat is
  `1 - (phase % 1)`. Negative while it waits for a later start;
- `state` — `0` playing, `1` waiting for its `after`, `2` starting on a later
  beat, `3` playing out its last pass (replaced by a new pattern with the same id).

A voice record: `level` is volume × envelope (how loud it is now, `0..1`);
`kind` is `0` an oscillator, `1` a sample, `2` a sung note; `releasing` is `1`
once its note-off has passed (it's fading out).

```lua
S = nil
function loop()
  S = sounds(S)
  local nch, nseq, nv = S[3], S[4], S[5]
  local at = 6 + nch
  for k = 0, nseq - 1 do
    local r = at + k * 9
    local id, ch, hits, phase = S[r], S[r + 1], S[r + 5], S[r + 6]
    local pulse = 1 - (phase % 1)  -- 1 on the beat, falling to 0
    -- draw sequence `id`'s row, bumped by pulse (or when hits changed)
  end
end
```

On the web, a game's Lua runs apart from the audio, and `sounds()` returns an
empty snapshot (all counts 0) for now. See also `seq`, `bpm`.

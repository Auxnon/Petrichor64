## sing

_sing a vowel at a pitch (retro formant-synthesis voice)_

```lua
---@type fun(vowel: string, freq: number, length?: number, channel?: integer)
function sing(vowel, freq, length, channel)
```

Plays one **sung note**: a harmonic-rich sawtooth "glottal" source at `freq`,
shaped by the formant filters of `vowel` into a vowel sound — the way old speech
chips (SAM, Votrax) sang. No recorded voice, all synthesized.

- `vowel` — `'a'`, `'e'`, `'i'`, `'o'`, or `'u'` (only the first character is
  read, so `'la'`, `'aah'`, and `'a'` all sing "ah"). Unknown → `'a'`.
- `freq` — pitch in Hz (like `note`).
- `length` — sustain seconds before release (default 1).
- `channel` — optional; omit to auto-allocate a voice (so overlapping `sing`
  calls harmonize).

Sequence `sing` calls over time to sing a phrase; call several at once for a
vocal chord.

```lua
-- an "aah" up a little scale
sing('a', 261.63, 0.4)
sing('e', 329.63, 0.4)
sing('i', 392.00, 0.4)

-- a sustained vowel chord (three voices at once)
sing('o', 220, 2)
sing('o', 277, 2)
sing('o', 330, 2)
```

The voice is a Phase-1 formant singer: pure vowels, no consonants yet. It shares
the 16-voice polyphony and mixer with `note`/`smpl`. See also `note`, `instr`.

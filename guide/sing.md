## sing

_sing a syllable at a pitch (retro formant-synthesis voice)_

```lua
---@type fun(syllable: string, freq: number, length?: number, channel?: integer)
function sing(syllable, freq, length, channel)
```

Plays one **sung syllable**: an optional consonant onset (a short noise burst)
followed by a harmonic-rich sawtooth "glottal" source at `freq`, shaped by the
vowel's formant filters — the way old speech chips (SAM, Votrax) sang. No
recorded voice, all synthesized.

- `syllable` — a vowel with an optional leading consonant:
  - **vowels**: `a` `e` `i` `o` `u` (the sung tone).
  - **consonants** (noise onset): `s` `f` `h` `t` `k` `p` (and voiced pairs
    `z` `v` `d` `g` `b`), plus the digraph `sh`. So `'sa'`, `'ta'`, `'shi'`,
    `'fu'` all work.
  - `l` `r` `m` `n` `w` `y` have no onset yet — they just sing the vowel.
- `freq` — pitch in Hz (like `note`).
- `length` — sustain seconds before release (default 1).
- `channel` — optional; omit to auto-allocate a voice (overlapping calls harmonize).

Sequence `sing` calls over time for a phrase; call several at once for a vocal chord.

```lua
-- "la la laa" up a little scale
sing('la', 261.63, 0.4)
sing('la', 329.63, 0.4)
sing('laa', 392.00, 0.8)

-- consonants: "sa ta sha"
sing('sa', 440, 0.4)
sing('ta', 440, 0.4)
sing('sha', 440, 0.4)

-- a sustained vowel chord (three voices at once)
sing('o', 220, 2)
sing('o', 277, 2)
sing('o', 330, 2)
```

Phase 1 voice: five vowels + unvoiced-consonant onsets. Voiced consonants and a
lyric/phoneme sequencer (vowel glides) are future work. Shares the 16-voice
polyphony and mixer with `note`/`smpl`. See also `note`, `instr`.

## sing

_sing a syllable or phrase over a melody (retro formant-synthesis voice)_

```lua
---@type fun(lyrics: string, melody: number|number[]|number[][], length?: number, channel?: integer)
function sing(lyrics, melody, length, channel)
```

Sings `lyrics` — one **syllable** (`"sa"`) or a space-separated **phrase**
(`"la la laa"`) — over `melody`. Each syllable is an optional consonant onset (a
short noise burst) then a harmonic-rich sawtooth "glottal" source shaped by the
vowel's formant filters — the way old speech chips (SAM, Votrax) sang. No
recorded voice, all synthesized.

- **`melody`** is one of:
  - a single pitch (number) — every syllable sings that pitch;
  - a table of pitches `{261, 329, 392}` — one per syllable;
  - a table of `{freq, len}` pairs — per-syllable pitch *and* length.
  - Fewer melody entries than syllables → the last one repeats.
- A **phrase** (multiple syllables) sequences on one channel, back-to-back (like
  `song`); a single syllable plays immediately (and overlapping calls harmonize).

- `syllable` — a vowel with an optional leading consonant:
  - **vowels**: `a` `e` `i` `o` `u` (the sung tone).
  - **consonants** (noise onset): `s` `f` `h` `t` `k` `p` (and voiced pairs
    `z` `v` `d` `g` `b`), plus the digraph `sh`. So `'sa'`, `'ta'`, `'shi'`,
    `'fu'` all work.
  - **voiced consonants** `m` `n` `l` `r` `w` `y` — tonal onsets that *glide*
    into the vowel (so `'la'`, `'ma'`, `'ra'` sound like a real syllable).
  - **diphthongs** — two vowels in one syllable glide between them: `'ai'`
    ("eye"), `'au'` ("ow"), `'oi'` ("oy").
- `length` — default per-syllable sustain seconds (used when `melody` doesn't
  give per-syllable lengths). Default 0.5.
- `channel` — optional; for a single syllable, omit to auto-allocate a voice
  (overlapping calls harmonize). A phrase sequences on one channel.

```lua
-- a phrase up a little scale (one syllable per pitch)
sing('la la laa', { 261.63, 329.63, 392.00 }, 0.4)

-- consonants, all on one pitch
sing('sa ta sha', 440, 0.4)

-- per-syllable pitch AND length via {freq, len} pairs
sing('do re mi', { {261,0.3}, {293,0.3}, {329,0.6} })

-- a single sustained vowel; several at once = a vocal chord
sing('o', 220, 2)
sing('o', 277, 2)
sing('o', 330, 2)
```

Phase: five vowels + unvoiced-consonant onsets + a phrase sequencer. Voiced
consonants (m/n/l/r glides) and vowel-to-vowel glides are future work. Shares the
16-voice polyphony and mixer with `note`/`smpl`. See also `note`, `song`, `fade`.

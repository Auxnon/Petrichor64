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

- `syllable` — a whole CVC(C) syllable: an optional onset consonant cluster, a
  vowel, and an optional coda cluster. So `'strong'`, `'cat'`, `'sun'` all sing.
  - **vowels**: `a` `e` `i` `o` `u` (the sung tone).
  - **consonants** (noise onset): `s` `f` `h` `t` `k` `p` (and voiced pairs
    `z` `v` `d` `g` `b`), plus digraphs `sh`/`ch`/`th`. So `'sa'`, `'ta'`, `'shi'`.
  - **voiced consonants** `m` `n` `l` `r` `w` `y` — tonal onsets that *glide*
    into the vowel (so `'la'`, `'ma'`, `'ra'` sound like a real syllable).
  - **onset clusters** — several consonants before the vowel play in order:
    `'st'`, `'tr'`, `'pl'`, `'str'`, `'spr'` (`'stop'`, `'tree'`, `'strong'`).
  - **codas** — consonants *after* the vowel: unvoiced bursts (`'cat'`, `'stop'`,
    `'cats'`, `'ask'`) and voiced endings that glide the vowel into a nasal/liquid
    (`'sun'`, `'call'`, `'him'`, `'sing'`); mixed too (`'sink'`, `'want'`).
  - **diphthongs** — two vowels in one syllable glide between them: `'ai'`
    ("eye"), `'au'` ("ow"), `'oi'` ("oy").
- `length` — default per-syllable sustain seconds (used when `melody` doesn't
  give per-syllable lengths). Default 0.5.
- `channel` — which channel ("track") to sing on. Default `0`. A phrase sequences
  on one lane of the channel; a single syllable takes a free lane, so overlapping
  `sing` calls on the same channel harmonize (a vocal chord). Set the channel's
  voice character with `vox(channel, …)`.

```lua
-- a phrase up a little scale (one syllable per pitch)
sing('la la laa', { 261.63, 329.63, 392.00 }, 0.4)

-- consonants, all on one pitch
sing('sa ta sha', 440, 0.4)

-- whole words: onset clusters + codas
sing('strong cat sun', { 262, 294, 330 }, 0.6)

-- per-syllable pitch AND length via {freq, len} pairs
sing('do re mi', { {261,0.3}, {293,0.3}, {329,0.6} })

-- a single sustained vowel; several at once = a vocal chord
sing('o', 220, 2)
sing('o', 277, 2)
sing('o', 330, 2)
```

Shares the channel/lane polyphony and mixer with `note`/`smpl` — a sung chord
uses one channel's lanes (see `attr{lanes}`), so the whole chord shares that
channel's `vox` character. See also `note`, `song`, `fade`, `vox`, `attr`.

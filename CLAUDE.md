# Petrichor64 — notes for agents

## Lua command naming (core `lua!` natives in `src/command.rs`)

Core Lua command names should be **3–4 letters**. This keeps the scripting API
terse and console-friendly (it's a fantasy-console engine).

- **Prefer a real word** at 3–4 letters: `cam`, `key`, `tex`, `note`, `tile`,
  `make`, `mute`, `lamp`, `fog`, `song`, `anim`, `attr`.
- **Fun / rootword shortcuts are fine** when they read well: `mus` = mouse
  (Latin *mūs*), `flr` = floor, `rnd` = random, `img` families, etc.
- If you genuinely can't find a good 3–4 letter name, make it **as short as
  possible and get the user's approval before adding it** — don't ship a long
  name silently.

New commands: add the `lua!("name", …)` native in `src/command.rs` **and** a
matching `guide/<name>.md` (the macro `include_bytes!`s it in debug builds, so a
missing doc fails the build).

Long names predating this rule (candidates to shorten with approval): the
tile family `gtile`/`ftile`/`dtile`/`istile`, plus `chord`, `instr`, `empty`.

## Sound & audio (`src/sound.rs`, natives in `src/command.rs`)

Audio is a **native-only, non-default feature** (`--features audio`, pulls
`cpal`). All sound code and the sound `lua!` natives are `#[cfg(feature =
"audio")]`; the natives compile as no-op stubs otherwise (hence the many
"unused variable" warnings in the no-audio build — expected, not a regression).

Architecture: a single cpal output stream owns all synth state on the audio
thread; the engine talks to it one-way over an `mpsc` `Sender<SoundCommand>`
(`core.singer`). The stream **outlives Lua reloads**, so state must be reset
explicitly — `SoundCommand::Reset` (sent from `async_load_app` on every load)
clears instruments, samples, the loaded-file bank, and all voices. If you add
persistent audio state, clear it in the `Reset` arm too.

- **Instruments and samples are keyed by integer id — keep it that way.** The
  mixer hot path is a fast index lookup (`FxHashMap<usize, …>`); do **not**
  move it to string keys. Names only exist in the load/bind staging.
- **`instr(id, spec, width?)`** — oscillator (waveform-name string) or additive
  (harmonic-amplitude table). **`smpl(id, data, base?)`** — sampler: `data` is
  either a PCM table (raw `-1..1`) or a **name string** binding a loaded file.
- **Waveforms** live in `WaveType` + `osc()`; **samples** go through
  `voice_out()` (pitch-resampled by `freq/base_freq`, linear interp, one-shots
  self-release). New sample buffers are loudness-matched via `normalize_pcm`.
- **Silt `Table`→`Vec` has a hash-order bug** (`to_vec`, still unfixed
  upstream): read PCM/chord/song arrays with a `getn(i)` index loop, never
  `Vec<f32>` `FromLua`, or the buffer scrambles into noise.

### Loading sound files (`sounds/` folder)

Games may have a `sounds/` folder alongside `assets/`/`scripts/`. `.ogg` files
there are decoded at boot (pure-Rust `lewton`, no C deps), normalized, and
stashed in a name→PCM bank keyed by filename stem (extensionless, like
textures/models). `smpl(id, 'name')` binds one into an integer slot — the
string lookup happens once, at bind time, never in the mixer. Buffers are
`Arc`-shared so binding one file to several slots doesn't copy.

The engine only **decodes** ogg. Converting `.wav`/`.mp3` → `.ogg` is the job
of the separate **`oggify`** CLI crate (keeps the Vorbis *encoder*'s C
dependency out of the size-optimized engine binary). See PLAN.md → Sound System.

Wiring lives in `src/asset.rs` (`decode_ogg`, `load_sounds_from_dir`,
`load_sounds_from_buffers`) and the load funnel `command.rs::async_load_app`.
Directory games load from disk; packed `.game.png` games decode the bundled
`sounds/` entries in `asset::unpack` (whitelist `["assets","scripts","sounds"]`).
**Known gap:** the *packer* (`collect_packable_sources`/`pack_folder`) does not
yet bundle `sounds/` into `.game.png`, so packed-game sound loading is untested.

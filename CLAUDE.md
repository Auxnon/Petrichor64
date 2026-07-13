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

# Lua command rename suggestions

Working toward the 3–4 letter core-command rule (see `CLAUDE.md`). `light` is
already renamed to `lamp`. Nothing here is applied yet — these need sign-off,
and the tile family especially is a **breaking API change** for existing games.

## Safe, isolated renames (recommended)

| current | len | suggestion | rationale |
|---|---|---|---|
| `mgrab` | 5 | `grab` | clean real word, name is free |
| `instr` | 5 | `inst` | "instrument"; `ins` if 3 preferred |

## Needs a decision

| current | len | options | notes |
|---|---|---|---|
| `chord` | 5 | keep / `crd` | real musical word; borderline |
| `empty` | 5 | `wipe` / `void` | confirm what it clears first |

## The tile family — pick a get/set/delete convention

These are the real design question. Currently: `tile` (set), `gtile` (get),
`ftile` (find), `dtile` (delete/destroy), `istile` (is/has) — 5–6 letters, and
inconsistent as a set.

Undecided direction (per Nick): readability vs. pico-8-style tweet-tiny code.

**Option A — namespaced table** (`tile.get`, `tile.set`, `tile.del`, `tile.find`,
`tile.is`): most readable, self-grouping, extends cleanly. Longer to type; needs
a `tile` table holding functions rather than flat globals.

**Option B — verb-prefix family** (`tile`=set, `tget`, `tset`, `tdel`, `tfnd`,
`thas`): flat globals, 4 letters, consistent `t*` prefix. Terse but a bit cryptic.

**Option C — ultra-short (pico-8 lean)**: single/double-letter ops, e.g. `tl`
(get), `ts` (set), `td` (del)… maximally terse, hardest to read.

Whatever the get/set/delete shape lands on, apply it **consistently across all
families** (there's a parallel `gimg`/`gmod` "get" pattern, and `nimg` for
"new"), so the convention reads the same everywhere:
- get: `gimg` (get image), `gmod` (get model), `gtile` (get tile)
- new: `nimg` (new image)

Deferred until the convention is decided.

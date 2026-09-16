## mon

_get or set the display monitor preset_

```lua
---@type fun(code?: string): string?
function mon(code)
```

Selects a CRT/LCD look for the final image — named after the underlying display
*technology*, not any brand:

- `"lcd"` — modern flat panel (default). Clean, no CRT artifacts.
- `"slot"` — slot-mask tube (Commodore-1084-ish). A softer, mid-ground CRT look:
  visible curvature, scanlines, some color bleed and glitch.
- `"grille"` — aperture-grille tube (Trinitron/PVM-ish). Crisp, punchy, low
  curvature and glitch — the "professional monitor" end of CRT looks.

Called with no argument, returns the current preset as a lowercase string.

Pairs with `chip()`, which controls the render *pipeline* (vertex/texture
behavior) independently of the monitor. Any individual `attr{}` field
(`curvature`, `dark`, `bleed`, `glitch`, etc.) can still be hand-tuned after
picking a monitor — the preset just sets sane starting values.

Resets to `"lcd"` on every app load, same as every other screen effect.

```lua
mon('slot')   -- softer consumer CRT
mon('grille') -- sharp pro CRT
mon('lcd')    -- back to modern
mon()         -- query, e.g. "slot"
```

### First-line header

A script's very first line can request an initial chip+monitor before `main()`
runs, as a single packed hex byte in a `--!` comment (a real Lua comment, so it
never breaks parsing):

```lua
--! 0x21
```

The byte is `(chip_nibble << 4) | monitor_nibble`:

| nibble | chip (high) | monitor (low) |
|---|---|---|
| 0 | r00 | lcd |
| 1 | r43 | slot |
| 2 | r30 | grille |

`0x21` above is R30 (high nibble `2`) + Slot (low nibble `1`). No header line,
`0x00`, or an unrecognized value all fall back to `r00` + `lcd` — today's
default, so every existing script is unaffected. The script's own `chip()`/
`mon()` calls still override the header afterward, same as `attr{}` can
override any boot-time default.

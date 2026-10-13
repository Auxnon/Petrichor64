# silt bugs found writing the editor overlay

> **Status re-checked 2026-10-10 while building `~/projects/model-chor`: #1, #2 and #3
> are NOT fixed in the silt the engine builds against**, and #8–#12 below are new.
> The engine uses `../silt-stable` on branch `fix/constant-index-u16` at `1315850`. The
> commits named as the fixes below (`9ac21ea`, `0e09035`) don't exist anywhere in that
> repo (`git cat-file -t` fails on both). So either they live in another checkout or
> they were never pushed. Until they land, treat the "fixed" line below as wrong and
> keep every workaround. The repros are in "Re-check" at the end.

> **#5 and #7 are open; #6 has a fix on branch `fix/constant-index-u16`** (#5 found building `apps/model`; #6 and #7 building `editor/`). #1–#4 are fixed in `../silt-stable` — #1 in `9ac21ea`, #2/#3 in `0e09035`, and #4
> along with them (the stack accounting it depended on). `test/silt-callarg` now
> reports `done, failures: 0` with no `is not callable` lines, and 60 top-level
> globals no longer panics. Kept for the reproductions and the diagnosis trail.

Three reproducible miscompiles, hit while building `apps/edit` (a source editor that
runs as an overlay). Each one is a silent wrong answer rather than a parse error,
which is what made them expensive to find — the reported line number points at
whatever ran *next*, not at the bad code.

Harness, in this repo:

```
cargo build --features audio
./target/debug/Petrichor64 test/silt-callarg
```

It prints `ok`/`FAIL` per check; a miscompile shows up as an `is not callable` error
line instead. Current output: **2 miscompiles, 4 FAILs**. When all three are fixed it
should print `done, failures: 0` with no error lines.

Paths below are in `../silt-stable` (the `path` dependency in `Cargo.toml`).

---

## 1. A call argument that is itself a call, followed by a local argument, calls the wrong thing

**Symptom.** The call invokes its own *first argument* instead of the function:

```lua
local y = 10
local col = "DDE"
text("lit", flr(8), y, col)   -- error: Value 'Value: "lit"' is not callable
```

`text` here is an ordinary global function. Same failure with method syntax
(`gui:text(...)`), so it isn't specific to the call sugar.

**What decides it.** The nested call has to be followed by at least one *local*
argument. Literals and globals in the same position are fine:

| call | result |
|---|---|
| `text(T[1], flr(8), 10, C)` — nested call, then literal + global | ok |
| `text(T[1], CW, y, col)` — locals, no nested call | ok |
| `text("lit", flr(8), y, col)` — nested call, then locals | **calls `"lit"`** |
| `text(T[1], flr(8), y, col)` — nested call, then locals | **calls `"a.lua"`** |
| `local x = flr(8); text(T[1], x, y, col)` — hoisted | ok |
| `text(T[1], flr(8), flr(y), C)` — two calls, then a global | ok |

**Where I think it is.** `compiler.rs::call` decides whether the *trailing* argument
was a call — Lua's open multiret — by looking at the last emitted opcode:

```rust
let trailing_multiret =
    !trailing_vararg && matches!(f.chunk.read_last_code(), OpCode::CALL(..));
```

If that misfires, the inner call is patched to `MULTIRET` and the outer call is
marked variadic, so its argument count is resolved at runtime in `lua.rs:2375`
`OpCode::CALL`:

```rust
let ar = if let Some(base) = pending_multiret.take() {
    (((self.stack_count - base) as u8).wrapping_add(*arity)).wrapping_sub(1)
```

…and the callee is then found by counting back from the top:

```rust
let value = self.peekn(ep, ar);
```

The observed callee is the first of four arguments, i.e. `ar` came out exactly **one
too low** — consistent with the `wrapping_sub(1)` multiret branch being taken for a
call whose trailing argument is *not* a call, with `stack_count - base == 0`.

My hypothesis is that `read_last_code()` is not a reliable "was the last argument a
call?" test for these argument shapes, so a nested call earlier in the list is
mistaken for a trailing one. I did not confirm why the local arguments after it fail
to displace it — `GET_LOCAL` does get emitted elsewhere (`compiler.rs:3005`), so
please verify rather than take my word for the mechanism.

**Suggested fix.** Have `arguments()` report whether the argument it just parsed was
a call expression, and use that flag in `call()` instead of inspecting the last
emitted opcode. Quickest way to confirm the diagnosis first: print `trailing_multiret`
while compiling the failing line and check it is wrongly `true`.

---

## 2. `table.insert(t, pos, v)` overwrites instead of shifting, on tables grown by index

**Symptom.** On a table built the way any line buffer or file list is built:

```lua
local t = {}
for i = 1, 19 do t[#t + 1] = "line" .. i end
table.insert(t, 2, "INS")
-- #t is 19, should be 20;  t[3] is "line3", so "line2" was destroyed
```

A table written as a `{ ... }` constructor behaves correctly, which is why this
survives small tests and breaks real code.

**Where it is.** `table.rs::insert` is written correctly, but bounds its shift loop
with `self.counter`:

```rust
let i = key.strict_int()?;
let mut k = self.counter;
while k >= i && k >= 1 { ... }
```

and `counter` is never maintained by plain index assignment. `t[k] = v` reaches
`Table::set` (`lua.rs:3166`), which is a bare `data.insert` and does not touch
`counter`. So on a loop-built table `counter` is still 0, the shift loop never runs,
and the element at `pos` is simply replaced.

`Table::set_and_check` *does* maintain `counter`, and it isn't the one being called
here — worth checking whether the VM should be routing index assignment through it.
Note its `recursion()` also seems unable to advance `counter` past the first element
of an empty table (`prev = 0` is never present), so it may need a fix of its own.

## 3. `table.remove(t, pos)` leaves a nil hole

Same root cause, same kind of table:

```lua
local t = {}
for i = 1, 19 do t[#t + 1] = "line" .. i end
table.remove(t, 2)
-- #t is 18 (right), but t[2] is nil and nothing shifted down
```

`table.rs::remove_at` closes the gap with `while k <= self.counter`, which does not
run when `counter` is 0. The entry is removed and the rest stay where they are, so
`t[i]` returns nil for an `i` well inside `1..#t`.

**Note the inconsistency behind both.** `#t` (`OpCode::LENGTH`) uses `Table::len()`,
which is `data.len()`, while `border()`, `insert`, `remove_at` and `pop` all use
`counter`. On a loop-built table those two disagree completely. Either `counter` has
to be maintained on integer-key assignment, or the array operations need to derive
their bounds from the real contents. `table.remove(t)` with no position is affected
too: it removes at `border()`, which is 0.

---

## 4. Not in the harness: `DEFINE_GLOBAL` panics with enough top-level globals

Around 60 top-level `NAME = value` definitions in one file aborts the Lua thread:

```
thread '<unnamed>' panicked at silt/src/lua.rs:1601:
attempt to subtract with overflow
```

That is `self.stack_count -= 1` in `OpCode::DEFINE_GLOBAL` running with
`stack_count` already 0. The inline pop next to it is fine — `Ephemeral::ip` is a
`*mut Value`, the stack pointer, not the instruction pointer — so the decrement is
the whole bug, but an underflow there means the stack accounting is already off by
the time it runs, which may share a cause with bug 1.

It is left out of `test/silt-callarg` because it kills the run. To reproduce, append
~60 blocks of this to any app's `main.lua`:

```lua
G1 = "constant"
H1 = "another"
function filler1(a)
	local s = G1 .. "-x"
	s = s .. H1
	if a == 1 then return s end
	return "no"
end
```

---

## 5. A table constructor holding a local, assigned into `t[#t + 1]`, corrupts the chunk

**Open.** Symptom is a compile-time failure of the whole file, reported at the
offending line:

```lua
local q = {}
local e = 8
q[#q + 1] = { -e, 0, 0 }   -- error: Invalid chunk due compilation corruption
```

The constructor has to contain a **local**; the same statement with constants is fine,
which is why it survives small tests:

| statement | result |
|---|---|
| `q[#q + 1] = { 1, 2, 3 }` | ok |
| `q[#q + 1] = 7` | ok |
| `q[1] = { -e, 0, 0 }` (constant index) | ok |
| `local n = #q + 1; q[n] = { -e, 0, 0 }` (index hoisted) | ok |
| `q[#q + 1] = { -e, 0, 0 }` | **corruption** |
| `q[#q + 1] = { a, 0, 0 }` where `a` is any local | **corruption** |
| `local r = { -e, 0, 0 }; q[#q + 1] = r` | ok |
| `table.insert(q, { -e, 0, 0 })` | ok |

It does not need a loop, and it is not about unary minus — `local n = -GRID` on its own
compiles fine. It is the combination of a constructor containing a local with the
`#t + 1` index expression. Both workarounds (hoist the constructor, or `table.insert`)
produce correct code, so the two operand paths clearly disagree about a register.

Found in a mesh builder — `q[#q + 1] = { a - LINE, -e, 0 }` in a loop appending quad
corners — which is about the most natural way to write that code, so it is worth
fixing rather than documenting. `apps/model` uses `table.insert` throughout with a
comment pointing here.

**Not just bracket-indexing — same failure on a plain field assignment inside a
loop.** Building `apps/art`, this also corrupted the chunk:

```lua
for i = 1, n do
	local size = ...
	ent.size = { size, 0.08, size }  -- ent is a userdata; .size is a field setter
end
```

with the same symptom (`Invalid chunk due compilation corruption`, reported line
number pointing at wherever execution happened to be next — in one repro, a
totally unrelated arithmetic statement several functions away, since the whole
chunk is one compiled unit and the report just names whatever ran into the
corrupted state first). The trigger is the same shape as the table case above —
a constructor holding a local, inside a loop — just via `obj.field = {...}`
instead of `t[expr] = {...}`. Same workaround applies: hoist the constructor into
a local first (`local sz = { size, 0.08, size }; ent.size = sz`). `apps/art`
does this throughout `build_buttons()`/`update_selection_marks()` with a comment
pointing here.

This makes the bug's real scope "a table constructor containing a local, written
inside a loop body, assigned anywhere" rather than specifically about `#t+1`
indexing — worth keeping in mind when re-testing a fix, so the fix is checked
against field assignment too, not just bracket indexing.

---

## 6. More than 256 constants in one chunk silently reads the wrong constants

**Open.** No error at compile time or at runtime — string and number literals just
resolve to *other* constants from the same chunk:

```lua
T = {}
T[1] = "s1"
T[2] = "s2"
-- ... one line per index, up to
T[200] = "s200"
return T[1] .. " " .. T[130] .. " " .. T[200]
-- expected: "s1 s130 s200"
-- actual:   "s73s73s74s74s75"   (even the " " separators are wrong)
```

Each line adds two constants (the index and the string), so it breaks at around 128
lines. It's per **chunk**: a script file's top level is one chunk, and each function
body is its own, so it bites data-heavy top-level code (asset tables, level data,
big lookup tables) long before it bites functions. The engine compiles each file in
`scripts/` separately, so splitting a file is a workaround.

**Where it is.** Every constant-bearing opcode stores its index as a `u8`
(`CONSTANT`, `CLOSURE`, `DEFINE_GLOBAL`, `GET_GLOBAL`, `SET_GLOBAL`,
`TABLE_GET_BY_CONSTANT`, `METHOD_GET`, … in `src/code.rs`), and the compiler
narrows with a silent cast:

```rust
// chunk.rs
pub fn write_constant(&mut self, value: Value<'chnk>) -> usize {
    self.constants.push(value);
    // TODO limit to u8
    self.constants.len() - 1
}
// compiler.rs
f.chunk.write_constant(value) as u8   // index 256 becomes 0, 257 becomes 1, ...
```

The misleading error line numbers seen alongside it (`Cannot get length of
'number'` reported on an unrelated line) come from the same thing — a wrapped
constant can be an identifier, so `GET_GLOBAL` fetches the wrong global.

Found while testing `editor/` with a harness that concatenated several scripts into
one chunk.

### Fix: widen constant operands to `u16` (branch `fix/constant-index-u16` in `../silt-stable`)

**Why `u16` is free.** `OpCode` is already **4 bytes, 2-byte aligned** (measured with
`size_of`): the jump variants (`GOTO_IF_FALSE(u16)`, `FOR_GENERIC { u8, u16 }`) set
the enum's size. A constant variant widened to `u16` is tag + padding + `u16` —
still 4 bytes. Same instruction stream size, same cache footprint, and the VM's
`constants[index as usize]` load is unchanged.

**What changed.**
- `code.rs`: `pub type ConstIndex = u16`; the 8 `constant:` operands use it
  (`CONSTANT`, `CLOSURE`, `DEFINE_GLOBAL`, `GET_GLOBAL`, `SET_GLOBAL`, `DEFINE_LOCAL`,
  `TABLE_GET_BY_CONSTANT`, `METHOD_GET`).
- `compiler.rs`: one checked narrowing, `Compiler::const_index`, used by every site
  that used to `as u8` (identifiers, literals, both `CLOSURE` emitters). Past
  `u16::MAX` it's a new compile error, `TooManyConstants`, reported at the
  offending line, instead of a wrap. `type Ident` (global-name index) follows.
- `lua.rs` hot-swap: `copy_constant(k as u8)` had the same wrap; now `ConstIndex`.
- `tests/constant_index.rs`: string literals, globals and closures past 256 resolve
  correctly; ~65 000 constants still run; overflowing is a clean compile error.
  All five fail on the old compiler.

**Speed.** Criterion `benches/interpreter.rs`, saved baseline vs widened:
for-loop −1.2%, while-loop 0.0%, fib(28) −0.3%, table r/w −3.2%, native call −1.1%,
native multi −3.1% — all noise or better. `string_concat` read +1.7…+6%, but an A/A
run (the *unchanged* code against the same baseline) read +6.2% too, so that's
machine drift, not the change. petrichor64 builds and its tests pass against it.

**Alternatives considered.**
- *clox-style `CONSTANT_LONG` variants* — keeps a u8 fast path, but doubles every
  constant-bearing opcode and its match arm in the VM, for no gain: the u16 is
  already free.
- *Error at 256* — tiny change, but breaks any big data script outright.
- *Deduplicate literal constants* — worth doing later (repeated numbers and strings
  each take a slot today; identifiers are already deduped), but it only delays the
  limit, and it's compile-time only.

**Related, not fixed here.**
- `Chunk::write_identifier` finds an existing identifier by *linear scan* of the
  constant table, so compiling a chunk with *n* globals is O(n²). Compile-time only;
  a `HashMap` side index (the existing `TODO` next to it) would fix it.
- `emit_rewind` / `emit_forloop` call `error_at(TooManyOperations)` and discard the
  result without `push_error`, so a >65535-op loop body marks the chunk invalid
  with no message ("Unknown error") and writes a wrapped jump first. Same shape of
  bug the new `const_index` avoids.
- Locals and upvalues are still `u8` slots (`TooManyLocals` guards them) — a real
  limit, but not a silent one.

---

## 7. silt CLI only: implicit returns crash on a global assigned from a call, then an `if` containing a call

**Open, does not affect the engine.** The `silt` binary compiles with
`Compiler::new_with_flags(true, true, false)` — `implicit_returns` on. petrichor64
uses `Compiler::new()` (off) and runs this fine.

```lua
function g() for i = 1, 2 do end return 1 end
function f() files = g() if false then print("none") end end
f() return files
-- silt CLI: panic in CallFrame::get_loc_by_count (src/function.rs:208)
-- or, in a bigger file, a segfault
```

All of these are needed: a loop in `g`, a **global** (not local) assigned from the
call, and an `if` as the last statement of `f` whose body contains a call. With
`if false then end` (no call) or `local x = g()` it runs. The likely suspect is the
implicit-return path in `compiler.rs` (`last_was_expression`, lines ~1142/2013)
treating the trailing `if` as a returnable expression and leaving the stack
unbalanced. Not investigated further, since it's CLI-only.

---

## What the workarounds cost (now removed)

`apps/edit` currently works around all three, and the workarounds are marked with
comments pointing back here, so they can be reverted once these are fixed:

- every computed argument is hoisted into a local before a draw call
- the line buffer is rebuilt on insert/delete instead of using `table.insert`/`table.remove`

Neither is expensive at this scale, but both make the code noticeably worse to read,
and the first is the kind of rule that is impossible to remember consistently.

---

## Re-check, 2026-10-10: what still reproduces (and what's new)

I ran these in the engine (`cargo build`, scripts in a scratch app) against
`../silt-stable` @ `1315850` on `fix/constant-index-u16`. Each test ran on its own
frame, so one failure couldn't hide the next. None of them are fixed.

### #1 still open: a call in the first argument, then a local later, calls the wrong value

The shape is wider than the table at the top says. **The call doesn't have to be
the whole argument.** A call anywhere inside the first argument's expression
triggers it, as long as a local appears later in the argument list:

| call | result |
|---|---|
| `two(1 + cos(az) * fl, 2)` | **`Value: 1` is not callable** |
| `two(cos(az) * fl, 2)` | **`Value: 0.7648…` is not callable** (calls the result of `cos`) |
| `three(cos(az), b, b)` | **not callable**, the exact case listed as fixed |
| `three(cos(az) * fl, b, b)` | **not callable** |
| `two(1 + cos(az), 2)` | ok (only a constant after the call) |
| `two(2, 1 + cos(az) * fl)` | ok (the call isn't in the first argument) |
| `three(b, cos(az) + 1, b)` | ok |
| `two(cos(az) * 2, sin(az) * 2)` | ok |
| `local q = cos(az) * 2 + 1` | ok |

The same thing happens in a table constructor. `{ 1 + cos(az) * fl, 2 }` fails with
`Invalid chunk due compilation corruption`.

**This is what breaks `apps/model` ("renders wrong").** Its `loop()` does
`vec3(cursor.x + cos(cam_az) * flat, …)`, so every frame errors with
`Value 'Value: 0.7648…' is not callable` and nothing after it runs. Workaround:
hoist every call into a local before using it as an argument.

### #2 / #3 still open: table.insert / table.remove on tables built by index

```lua
local t2 = {}
for i = 1, 5 do t2[#t2 + 1] = i end
table.remove(t2, 2)    -- t2[2] is nil (expected 3); #t2 is 4
table.insert(t2, 1, 9) -- t2[2] is nil (expected 1); #t2 is 4 (expected 5)
```

Two related things, same tables:

- **#8. `t[#t] = nil` doesn't shrink `#t`.** A 5-element loop-built table still
  reads `#t == 5` afterwards. HANDOFF mentions this one; it's still true.
- **#9. Setting a key to nil leaves it in `pairs`.** After `set[5] = true; set[5] = nil`,
  `for k in pairs(set)` still runs once. `set[5] == nil` reads correctly; only
  iteration is wrong.

Workaround: never delete. Rebuild the array with a filtering loop, and keep "sets"
as `true`/`false` values checked with `== true`.

### #10 new: anonymous functions panic the compiler

```lua
local f = function(q) return q * 2 end   -- inside a function body
```

This panics the compile with `attempt to subtract with overflow` at
`silt-stable/src/compiler.rs:1710` (local resolution: `i - offset` under a
functional depth). `m.k = function(self, q) … end` and
`pcall(function() error("x") end)` in the same function do the same, and the whole
file then fails to load. It also showed up at file top level: a table of
`TEST_FNS.x = function() … end` entries broke the chunk. Workaround: only named
global functions (`pcall(named_fn, arg)` works).

**Also: a one-line function body that declares a local miscompiles.**
`function f(d) local t = clock() g(d) ACC.x = ACC.x + clock() - t end` passed the
wrong values: `g` got an integer where it should have got a table, and another
copy gave `Cannot - 'number' and 'bool'`. The same body split across lines works.

### #11 new: assigning a field through an index fails when the element is userdata

```lua
gizmo_ents[k].z = -1000   -- error: Cannot perform table operations on a non-table value (userdata)
```

Reading `gizmo_ents[k].z` works, and so does `local e = gizmo_ents[k]; e.z = -1000`.
Only the set through an indexed expression fails, and only for userdata (entities).
Doing the same to a plain table element (`F[i].c = c`) is fine.

### #12 new: smaller gaps

- **No scientific notation.** `1e30` is a lex error (`Invalid number: 1`).
- **vec3 fields are read-only.** `v.x = 5` gives `Meta method missing for '[]'`.
  Build a new `vec3` instead.
- `vec3` math is f32. `vec3(3,0,4):normalize().x` is `0.6000000238…`, so don't
  compare it to an exact `0.6`. Expected behaviour, but it surprises an exact test.

### Checked and fine in this build

`+=`, `tonumber` (with base), `string.find/sub/format/match`, `pcall` with a named
function, multiple return values, `ipairs`/`pairs` (except #9), nested
constructors with constants, `table.concat`, vec3 `cross`/`dot`/`length`/arithmetic
and `==`, numeric `for` with a negative step.


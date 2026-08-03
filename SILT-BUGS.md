# silt bugs found writing the editor overlay

> **All fixed** in `../silt-stable` — #1 in `9ac21ea`, #2/#3 in `0e09035`, and #4
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

## What the workarounds cost (now removed)

`apps/edit` currently works around all three, and the workarounds are marked with
comments pointing back here, so they can be reverted once these are fixed:

- every computed argument is hoisted into a local before a draw call
- the line buffer is rebuilt on insert/delete instead of using `table.insert`/`table.remove`

Neither is expensive at this scale, but both make the code noticeably worse to read,
and the first is the kind of rule that is impossible to remember consistently.

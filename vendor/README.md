# Vendored crates

## winit 0.30.13

From crates.io, with one change, in `src/platform_impl/android/mod.rs`
(`pointer_force`, at the end of the file): a stylus or eraser touch reports its
`force` as `Force::Calibrated { altitude_angle: Some(..), .. }`, as winit does
for an Apple Pencil on iOS, instead of `Force::Normalized`. Upstream winit 0.30
drops Android's tool type, so without this a pen is indistinguishable from a
finger. The engine reads it in `Global::track_touch` (`touches()`'s `pen`).

Used through `[patch.crates-io]` in the engine's Cargo.toml. To move to a newer
winit: copy its crate source here, drop `Cargo.lock` and `.cargo-ok`, and redo
the change above (or drop the patch, if winit has learned tool types: 0.31's
pointer events have a pen source).

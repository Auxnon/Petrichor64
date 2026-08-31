# sounds/

Drop `.ogg` files here. They're decoded and loudness-normalized at load, keyed
by filename (extensionless — `footstep.ogg` → `'footstep'`), then bound into an
instrument slot from Lua:

```lua
smpl(3, 'footstep')   -- bind sounds/footstep.ogg into instrument slot 3
note(440, 1, nil, 3)  -- play it (base pitch defaults to 440 = natural rate)
```

Only `.ogg` loads at runtime. Convert `.wav`/`.mp3` to `.ogg` with the `oggify`
tool: `cargo run -p oggify -- <this game dir>`.

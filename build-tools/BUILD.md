# ANY

- Game `cargo build --release --features=include_auto`
- Engine `cargo build --release`
- Engine+Online `cargo build --release --features=online_capable`

run binyaren to reduce binary size even further?

# WINDOWS

PS: `& "C:\Program Files (x86)\Windows Kits\10\bin\10.0.19041.0\x86\rc.exe" assets\resources.rc`

# MAC

PreReq: `cargo install cargo-bundle`

- Game `bash
cargo bundle --features=include_auto --release
codesign -s "Apple Development: nicholasmcavoy89@gmail.com (M7KS95955P)" target/release/bundle/osx/Petrichor64.app/ `

# LINUX / STEAMDECK

(Currently need windows WSL, if only because glibc version is too new to work on SteamDeck as of September 2022, WSL has an older version. A downgraded version of linux may work too)

PreReq:

```bash
cargo install cargo-appimage
sudo apt install libfontconfig1-dev
```

- Game `cargo appimage --features=include_auto`
- Engine `cargo appimage`

`lua54, lua53, lua52, lua51, luajit, luajit52, luau`

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

- Studio AppImage: `just appimage` (`build-tools/appimage.sh`), into `target/appimage`
- Terminal `chor` as a Flatpak: `just flatpak-tui` (`build-tools/flatpak-tui.sh`), into `target/flatpak`
- A game baked in: `cargo build --release --features include_auto`, then `build-tools/appimage.sh target/release/chor`

An AppImage runs on a glibc at least as new as the one it was built against, so
build on an old distro. CI (`.github/workflows/packages.yml`) uses ubuntu-22.04.

`lua54, lua53, lua52, lua51, luajit, luajit52, luau`

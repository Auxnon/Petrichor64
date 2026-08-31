fn main() {
    // println!("cargo:rustc-link-arg=assets/resources.res");

    // `desktop`: a native target with the things a desktop OS has and a phone
    // doesn't — a clipboard, native file/message dialogs, a terminal. Native is not
    // the same thing as desktop once Android and iOS are targets, and writing
    // `not(any(wasm32, android, ios))` at every call site both reads badly and
    // drifts. This names it once.
    //
    // Deliberately *not* the same mechanism as the dependency tables in Cargo.toml:
    // build-script cfgs can't drive dependency resolution, so the desktop-only
    // crates (clipboard, native-dialog, crossterm, midir) are gated there by the
    // equivalent `cfg(not(any(...)))` predicate written out longhand. The two must
    // agree — if you add a target here, add it there too.
    println!("cargo::rustc-check-cfg=cfg(desktop)");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let desktop = arch != "wasm32"
        && matches!(
            os.as_str(),
            "macos" | "windows" | "linux" | "freebsd" | "dragonfly" | "netbsd" | "openbsd"
        );
    if desktop {
        println!("cargo::rustc-cfg=desktop");
    }
}

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

    // Before the embeds: their nested builds reuse the checkout this fetches.
    if std::env::var_os("CARGO_FEATURE_STUDIO").is_some() {
        image_chor::run();
    } else {
        image_chor::write_empty();
    }

    // Not for the wasm build itself, which build_web below runs from inside
    // this script: that would recurse.
    if std::env::var_os("CARGO_FEATURE_WEB_EMBED").is_some() && arch != "wasm32" {
        web_embed::run();
    }
    // The TUI is a terminal app (crossterm), so desktop targets only. The nested
    // build below doesn't enable `tui-embed`, so it can't recurse.
    if std::env::var_os("CARGO_FEATURE_TUI_EMBED").is_some() && desktop {
        tui_embed::run();
    }
}

/// A `Command` for a cargo-driven tool run from inside this build script, for a
/// separate build of this crate. The nested cargo gets its own target dir: the
/// outer build holds a lock on target/, and sharing it would deadlock.
fn nested_command(program: &str, manifest: &std::path::Path, target_dir: &std::path::Path) -> std::process::Command {
    let mut c = std::process::Command::new(program);
    // This script's env describes the outer build (CARGO_CFG_*, CARGO_FEATURE_*,
    // TARGET, flags...). The nested cargo passes inherited env on to its own build
    // scripts, so leftovers either compile deps for the wrong OS (CARGO_CFG_UNIX
    // on wasm) or recurse (CARGO_FEATURE_WEB_EMBED). Keep only the user's cargo
    // config.
    const KEEP: [&str; 5] =
        ["CARGO_HOME", "CARGO_NET_", "CARGO_HTTP_", "CARGO_REGISTRIES_", "CARGO_REGISTRY_"];
    for (key, _) in std::env::vars_os() {
        let k = key.to_string_lossy();
        if k.starts_with("CARGO_") && !KEEP.iter().any(|keep| k.starts_with(keep)) {
            c.env_remove(&key);
        }
    }
    for key in ["TARGET", "HOST", "PROFILE", "OPT_LEVEL", "DEBUG", "NUM_JOBS", "OUT_DIR", "RUSTFLAGS"] {
        c.env_remove(key);
    }
    c.current_dir(manifest).env("CARGO_TARGET_DIR", target_dir);
    // The nested build compiles the same image editor this one did, without
    // fetching it again.
    if let Some(dir) = image_chor::fetched() {
        c.env(image_chor::DIR_ENV, dir);
    }
    c
}

/// `studio`: the image editor (the Ctrl+P paint overlay) lives in its own repo,
/// github.com/Auxnon/image-chor. Fetch its latest `main` and compile its
/// `scripts/*.lua` into the engine (src/builtin.rs includes the generated
/// $OUT_DIR/image_chor.rs). This runs again whenever the engine's sources
/// change, so any rebuild picks up the newest version; `just image-chor` forces
/// one.
///
/// - `IMAGE_CHOR_DIR`: use this local checkout instead; nothing is fetched.
/// - `IMAGE_CHOR_REF`: a branch, tag or commit instead of `main`.
/// - Offline, the copy fetched last time is used. With none at all the build
///   still succeeds, with a warning, and Ctrl+P says the editor isn't built in.
mod image_chor {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    pub const DIR_ENV: &str = "IMAGE_CHOR_DIR";
    const REF_ENV: &str = "IMAGE_CHOR_REF";
    const URL: &str = "https://github.com/Auxnon/image-chor";

    fn checkout() -> PathBuf {
        PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("image-chor")
    }

    /// The checkout this build compiled in, once run() has fetched it.
    pub fn fetched() -> Option<PathBuf> {
        std::env::var_os("CARGO_FEATURE_STUDIO")?;
        let dir = std::env::var_os(DIR_ENV).map(PathBuf::from).unwrap_or_else(checkout);
        dir.join("scripts").is_dir().then_some(dir)
    }

    pub fn run() {
        println!("cargo::rerun-if-env-changed={}", DIR_ENV);
        println!("cargo::rerun-if-env-changed={}", REF_ENV);
        let dir = match std::env::var_os(DIR_ENV) {
            Some(dir) => {
                // Absolute, since the generated include_str!s resolve from OUT_DIR.
                let dir = std::fs::canonicalize(&dir).unwrap_or_else(|_| PathBuf::from(dir));
                println!("cargo::rerun-if-changed={}", dir.join("scripts").display());
                dir
            }
            None => {
                // Printing any rerun-if-changed replaces cargo's default of
                // rerunning on every change in the package, so name the inputs
                // that should trigger a fetch: the engine's own sources.
                for input in ["build.rs", "Cargo.toml", "src"] {
                    println!("cargo::rerun-if-changed={}", input);
                }
                let dir = checkout();
                let want = std::env::var(REF_ENV).unwrap_or_else(|_| "main".into());
                if let Err(e) = fetch(&dir, &want) {
                    let old = if dir.join("scripts").is_dir() { "; using the copy fetched last time" } else { "" };
                    println!("cargo::warning=image-chor: couldn't fetch {} from {}: {}{}", want, URL, e, old);
                }
                dir
            }
        };
        write(&dir);
    }

    fn fetch(dir: &Path, want: &str) -> Result<(), String> {
        if !dir.join(".git").is_dir() {
            git(None, &["init", "-q", &dir.to_string_lossy()])?;
        }
        git(Some(dir), &["fetch", "-q", "--depth", "1", URL, want])?;
        git(Some(dir), &["checkout", "-q", "--force", "--detach", "FETCH_HEAD"])?;
        Ok(())
    }

    fn git(dir: Option<&Path>, args: &[&str]) -> Result<String, String> {
        let mut c = Command::new("git");
        if let Some(dir) = dir {
            c.arg("-C").arg(dir);
        }
        // Never stop the build to ask for credentials.
        c.args(args).env("GIT_TERMINAL_PROMPT", "0");
        let out = c.output().map_err(|e| format!("git: {}", e))?;
        if out.status.success() {
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().replace('\n', " "))
        }
    }

    fn write(dir: &Path) {
        let mut scripts: Vec<PathBuf> = std::fs::read_dir(dir.join("scripts"))
            .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).collect())
            .unwrap_or_default();
        scripts.retain(|p| p.extension().is_some_and(|x| x == "lua"));
        scripts.sort();
        if scripts.is_empty() {
            println!("cargo::warning=image-chor: no scripts in {}; building without the image editor", dir.display());
        }
        let mut src = String::from("pub static PAINT: &[(&str, &str)] = &[\n");
        for path in &scripts {
            let name = path.file_name().unwrap().to_string_lossy();
            src += &format!("    ({:?}, include_str!({:?})),\n", name, path.to_string_lossy());
        }
        src += "];\n";
        let rev = git(Some(dir), &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
        src += &format!("pub const PAINT_REV: &str = {:?};\n", rev);
        emit(&src);
    }

    /// Without `studio`: no image editor.
    pub fn write_empty() {
        emit("pub static PAINT: &[(&str, &str)] = &[];\npub const PAINT_REV: &str = \"\";\n");
    }

    fn emit(src: &str) {
        let path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("image_chor.rs");
        // Only when it changed, so an unchanged editor doesn't look new to cargo.
        if std::fs::read_to_string(&path).ok().as_deref() != Some(src) {
            std::fs::write(&path, src).unwrap();
        }
    }
}

/// `web-embed` (part of `kitchensink`): build the web (wasm) version of the
/// engine and embed it, gzipped, so `chor -w <game dir>` serves it with nothing
/// on disk. The files stay gzipped in the binary and go out with
/// `Content-Encoding: gzip`, so `chor` carries no decompressor.
mod web_embed {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    pub fn run() {
        let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());

        println!("cargo::rerun-if-env-changed=CHOR_WEB_DIST");
        let dist = match std::env::var_os("CHOR_WEB_DIST") {
            Some(dir) => PathBuf::from(dir),
            None => {
                // Only the inputs to the web build: listing web/ whole would
                // include web/synth, which wasm-pack rewrites on every build,
                // so it would never settle.
                for input in [
                    "build.rs", "Cargo.toml", "Cargo.lock", "Trunk.toml", "src", "synth/src",
                    "synth/Cargo.toml", "web/index.html", "web/worker.js", "web/synth-worklet.js",
                    "web/worklet-polyfill.js", "web/petrichor64-element.js", "web/default.game.png",
                ] {
                    println!("cargo::rerun-if-changed={}", input);
                }
                build(&manifest, &out)
            }
        };

        let gz_dir = out.join("web-gz");
        let _ = std::fs::remove_dir_all(&gz_dir);
        let mut files = Vec::new();
        collect(&dist, &dist, &mut files);
        if !files.iter().any(|f| f == "index.html") {
            panic!("web-embed: {} has no index.html", dist.display());
        }
        let mut table = String::from("pub static FILES: &[(&str, &[u8])] = &[\n");
        for rel in &files {
            let gz = gz_dir.join(format!("{}.gz", rel));
            std::fs::create_dir_all(gz.parent().unwrap()).unwrap();
            gzip(&dist.join(rel), &gz);
            table.push_str(&format!("    ({:?}, include_bytes!({:?})),\n", rel, gz));
        }
        table.push_str("];\n");
        std::fs::write(out.join("web_embed.rs"), table).unwrap();
    }

    /// Run the same steps as `just web-build`, into OUT_DIR.
    fn build(manifest: &Path, out: &Path) -> PathBuf {
        let missing: Vec<&str> = ["trunk", "wasm-pack"]
            .into_iter()
            .filter(|tool| Command::new(tool).arg("--version").output().is_err())
            .collect();
        if !missing.is_empty() {
            panic!(
                "web-embed (kitchensink) builds the web version first and needs {}.\n\
                 Install: cargo install trunk wasm-pack && rustup target add wasm32-unknown-unknown\n\
                 Or point CHOR_WEB_DIST at an already-built web dir (e.g. web/dist from `just web-build`).",
                missing.join(" and ")
            );
        }

        let target_dir = out.join("web-target");
        let dist = out.join("web-dist");
        let nested = |program: &str| super::nested_command(program, manifest, &target_dir);

        // The AudioWorklet's own small wasm module; web/index.html copies it
        // in from web/synth/. Mirrors the justfile's `synth-wasm`.
        exec(nested("wasm-pack").args([
            "build", "synth", "--release", "--target", "web", "--out-dir", "../web/synth",
            "--", "--no-default-features", "--features", "worklet",
        ]));
        exec(nested("trunk").args(["build", "--release", "--dist"]).arg(&dist));
        dist
    }

    fn exec(cmd: &mut Command) {
        let status = cmd.status().unwrap_or_else(|e| panic!("web-embed: {:?}: {}", cmd, e));
        if !status.success() {
            panic!("web-embed: {:?} failed ({})", cmd, status);
        }
    }

    fn collect(root: &Path, dir: &Path, files: &mut Vec<String>) {
        let mut entries: Vec<_> = std::fs::read_dir(dir)
            .unwrap_or_else(|e| panic!("web-embed: {}: {}", dir.display(), e))
            .flatten()
            .map(|e| e.path())
            .collect();
        entries.sort();
        for path in entries {
            let name = path.file_name().unwrap().to_string_lossy();
            // trunk's staging dir, wasm-pack's catch-all .gitignore, etc.
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                collect(root, &path, files);
            } else {
                let rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                files.push(rel);
            }
        }
    }

    fn gzip(src: &Path, dst: &Path) {
        use std::io::Write;
        let data = std::fs::read(src).unwrap();
        let mut enc = flate2::write::GzEncoder::new(
            std::fs::File::create(dst).unwrap(),
            flate2::Compression::best(),
        );
        enc.write_all(&data).unwrap();
        enc.finish().unwrap();
    }
}

/// `tui-embed` (part of `kitchensink`): build the terminal (render-tui) version of
/// the engine and embed it, gzipped, so `chor -t <game dir>` can run a game in the
/// terminal. `headed` and `render-tui` can't share a binary (each is its own
/// build of the core), so this is a second build of the same crate, unpacked to a
/// cache dir and run on first use (see src/tui_embed.rs).
mod tui_embed {
    use std::path::PathBuf;

    /// The TUI build's features: default minus `headed` (and `audio`: the
    /// non-headed core has no sound thread, so sound calls would error rather
    /// than no-op).
    const FEATURES: &str = "silt,studio,bin,render-tui";

    pub fn run() {
        let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let target = std::env::var("TARGET").unwrap();
        let exe = if target.contains("windows") { "chor.exe" } else { "chor" };

        println!("cargo::rerun-if-env-changed=CHOR_TUI_BIN");
        let bin = match std::env::var_os("CHOR_TUI_BIN") {
            Some(path) => PathBuf::from(path),
            None => {
                for input in ["build.rs", "Cargo.toml", "Cargo.lock", "src"] {
                    println!("cargo::rerun-if-changed={}", input);
                }
                let target_dir = out.join("tui-target");
                // Always release: this is the binary people run, whatever the
                // outer profile. `--target` keeps a cross build's TUI on the same
                // target as the chor that carries it.
                let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
                let mut cmd = super::nested_command(&cargo, &manifest, &target_dir);
                cmd.args(["build", "--release", "--bin", "chor", "--no-default-features"])
                    .args(["--features", FEATURES, "--target", &target]);
                let status = cmd
                    .status()
                    .unwrap_or_else(|e| panic!("tui-embed: {:?}: {}", cmd, e));
                if !status.success() {
                    panic!("tui-embed: {:?} failed ({})", cmd, status);
                }
                target_dir.join(&target).join("release").join(exe)
            }
        };

        let data = std::fs::read(&bin)
            .unwrap_or_else(|e| panic!("tui-embed: {}: {}", bin.display(), e));
        let gz = out.join("chor-tui.gz");
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        std::io::Write::write_all(&mut enc, &data).unwrap();
        std::fs::write(&gz, enc.finish().unwrap()).unwrap();

        // Names the unpacked copy, so a rebuilt chor never runs a stale TUI.
        let mut hash: u64 = 0xcbf29ce484222325;
        for b in &data {
            hash = (hash ^ *b as u64).wrapping_mul(0x100000001b3);
        }
        std::fs::write(
            out.join("tui_embed.rs"),
            format!(
                "pub static TUI_GZ: &[u8] = include_bytes!({:?});\npub static TUI_ID: &str = \"{:016x}\";\n",
                gz, hash
            ),
        )
        .unwrap();
    }
}

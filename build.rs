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

    // Not for the wasm build itself, which build_web below runs from inside
    // this script: that would recurse.
    if std::env::var_os("CARGO_FEATURE_WEB_EMBED").is_some() && arch != "wasm32" {
        web_embed::run();
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

    /// Run the same steps as `just web-build`, into OUT_DIR. The nested cargo
    /// gets its own target dir: the outer build holds a lock on target/, and
    /// sharing it would deadlock.
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
        let nested = |program: &str| {
            let mut c = Command::new(program);
            // This script's env describes the outer, native build (CARGO_CFG_*,
            // CARGO_FEATURE_*, TARGET, flags...). The nested cargo passes
            // inherited env on to its own build scripts, so leftovers either
            // compile deps for the wrong OS (CARGO_CFG_UNIX on wasm) or recurse
            // (CARGO_FEATURE_WEB_EMBED). Keep only the user's cargo config.
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
            c.current_dir(manifest).env("CARGO_TARGET_DIR", &target_dir);
            c
        };

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

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
    for ed in editors::ALL {
        if std::env::var_os("CARGO_FEATURE_STUDIO").is_some() {
            ed.run();
        } else {
            ed.write_empty();
        }
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
    // The nested build compiles the same editors this one did, without
    // fetching them again.
    for ed in editors::ALL {
        if let Some(dir) = ed.fetched() {
            c.env(ed.dir_env, dir);
        }
    }
    c
}

/// `studio`: the editors that open over a running app live in their own repos
/// and are compiled into the engine as overlays (src/builtin.rs includes the
/// generated $OUT_DIR/<name>.rs):
///
/// - the image editor (Ctrl+P), github.com/Auxnon/image-chor
/// - the model editor (Ctrl+M), github.com/Auxnon/model-chor
///
/// Each is a git clone in the engine's own folder (`image-chor/`,
/// `model-chor/`, both gitignored), made by the first build that doesn't find
/// it; after that the build never fetches, it compiles whatever that checkout
/// holds. Its `scripts/` (`.lua` and `.silt`; `ignore.lua`, the LSP stubs, is
/// left out) go in, and editing them, pulling, or switching branches there
/// rebuilds. `just editors` pulls both.
///
/// - `IMAGE_CHOR_DIR` / `MODEL_CHOR_DIR`: use this checkout instead.
/// - `IMAGE_CHOR_REF` / `MODEL_CHOR_REF`: the branch, tag or commit the first
///   clone checks out instead of `main`. An existing clone is left as it is.
/// - Offline with no clone yet, the build still succeeds, with a warning, and
///   the shortcut says the editor isn't built in; the next build tries again.
mod editors {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    pub struct Editor {
        /// repo name, and the folder it's cloned into in the engine's folder
        name: &'static str,
        pub dir_env: &'static str,
        ref_env: &'static str,
        url: &'static str,
        /// the generated file's statics: the script list and the commit
        scripts_static: &'static str,
        rev_const: &'static str,
    }

    pub const ALL: [Editor; 2] = [
        Editor {
            name: "image-chor",
            dir_env: "IMAGE_CHOR_DIR",
            ref_env: "IMAGE_CHOR_REF",
            url: "https://github.com/Auxnon/image-chor",
            scripts_static: "PAINT",
            rev_const: "PAINT_REV",
        },
        Editor {
            name: "model-chor",
            dir_env: "MODEL_CHOR_DIR",
            ref_env: "MODEL_CHOR_REF",
            url: "https://github.com/Auxnon/model-chor",
            scripts_static: "MODEL",
            rev_const: "MODEL_REV",
        },
    ];

    impl Editor {
        fn checkout(&self) -> PathBuf {
            PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join(self.name)
        }

        /// The checkout this build compiled in, once run() has made sure of it.
        pub fn fetched(&self) -> Option<PathBuf> {
            std::env::var_os("CARGO_FEATURE_STUDIO")?;
            let dir = std::env::var_os(self.dir_env).map(PathBuf::from).unwrap_or_else(|| self.checkout());
            dir.join("scripts").is_dir().then_some(dir)
        }

        pub fn run(&self) {
            println!("cargo::rerun-if-env-changed={}", self.dir_env);
            println!("cargo::rerun-if-env-changed={}", self.ref_env);
            let dir = match std::env::var_os(self.dir_env) {
                Some(dir) => {
                    // Absolute, since the generated include_str!s resolve from OUT_DIR.
                    let dir = std::fs::canonicalize(&dir).unwrap_or_else(|_| PathBuf::from(dir));
                    println!("cargo::rerun-if-changed={}", dir.join("scripts").display());
                    dir
                }
                None => {
                    let dir = self.checkout();
                    if !dir.join("scripts").is_dir() {
                        let want = std::env::var(self.ref_env).unwrap_or_else(|_| "main".into());
                        if let Err(e) = clone(&dir, self.url, &want) {
                            println!("cargo::warning={}: couldn't clone {} from {}: {}", self.name, want, self.url, e);
                        }
                    }
                    // Rebuild when its scripts change: an edit, a pull, another
                    // branch. (While there's no clone, cargo reruns this every
                    // build, which tries the clone again.)
                    println!("cargo::rerun-if-changed={}", dir.join("scripts").display());
                    dir
                }
            };
            self.write(&dir);
        }

        fn write(&self, dir: &Path) {
            let mut scripts: Vec<PathBuf> = std::fs::read_dir(dir.join("scripts"))
                .map(|rd| rd.filter_map(|e| e.ok()).map(|e| e.path()).collect())
                .unwrap_or_default();
            scripts.retain(|p| {
                let script = p.extension().is_some_and(|x| x == "lua" || x == "silt");
                // ignore.lua is `chor ignore`'s API stubs for the LSP: empty
                // functions that would shadow the real natives
                script && !p.to_string_lossy().ends_with("ignore.lua")
            });
            scripts.sort();
            if scripts.is_empty() {
                println!("cargo::warning={}: no scripts in {}; building without it", self.name, dir.display());
            }
            let mut src = format!("pub static {}: &[(&str, &str)] = &[\n", self.scripts_static);
            for path in &scripts {
                let name = path.file_name().unwrap().to_string_lossy();
                src += &format!("    ({:?}, include_str!({:?})),\n", name, path.to_string_lossy());
            }
            src += "];\n";
            let rev = git(Some(dir), &["rev-parse", "--short", "HEAD"]).unwrap_or_default();
            src += &format!("pub const {}: &str = {:?};\n", self.rev_const, rev);
            self.emit(&src);
        }

        /// Without `studio`: not built in.
        pub fn write_empty(&self) {
            let src = format!(
                "pub static {}: &[(&str, &str)] = &[];\npub const {}: &str = \"\";\n",
                self.scripts_static, self.rev_const
            );
            self.emit(&src);
        }

        fn emit(&self, src: &str) {
            let file = format!("{}.rs", self.name.replace('-', "_"));
            let path = PathBuf::from(std::env::var("OUT_DIR").unwrap()).join(file);
            // Only when it changed, so an unchanged editor doesn't look new to cargo.
            if std::fs::read_to_string(&path).ok().as_deref() != Some(src) {
                std::fs::write(&path, src).unwrap();
            }
        }
    }

    /// A full clone, so it can be pulled, edited and committed to like any
    /// other checkout.
    fn clone(dir: &Path, url: &str, want: &str) -> Result<(), String> {
        if dir.exists() {
            return Err(format!("{} is there but has no scripts/; move it aside to clone again", dir.display()));
        }
        git(None, &["clone", "-q", url, &dir.to_string_lossy()])?;
        if want != "main" {
            git(Some(dir), &["checkout", "-q", want])?;
        }
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

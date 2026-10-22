//! The CLI flags and subcommands every desktop `chor` has, headed or
//! terminal-only, handled before the engine starts:
//!  - `chor --help|-h` and `chor --version|-v`.
//!  - `chor ignore [dir|-] [--yes|-y]`: write the `ignore.lua` an LSP reads
//!    for the engine's commands. Inside a game (a folder with `scripts/`, or
//!    anywhere under one) it goes in its `scripts/`, replacing the one there
//!    once you agree after seeing both codex versions; elsewhere it's written
//!    into `dir` (default `.`). `-` prints it instead.
#![cfg(not(target_arch = "wasm32"))]

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Returns true if a flag or subcommand was handled, so `main()` should exit
/// instead of starting the engine.
pub fn run_tool_cli() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print!("{}", help());
        return true;
    }
    if args.iter().any(|a| a == "--version" || a == "-v") {
        println!("chor {} ({})", VERSION, codex_name());
        if !crate::builtin::PAINT_REV.is_empty() {
            println!("image-chor {}", crate::builtin::PAINT_REV);
        }
        if !crate::builtin::MODEL_REV.is_empty() {
            println!("model-chor {}", crate::builtin::MODEL_REV);
        }
        return true;
    }
    if args.first().map(|a| a.as_str()) == Some("ignore") {
        if let Err(e) = ignore(&args[1..]) {
            eprintln!("chor ignore: {}", e);
            std::process::exit(1);
        }
        return true;
    }
    false
}

fn help() -> String {
    let mut s = format!(
        "chor {VERSION}: the Petrichor64 fantasy console ({})\n\nusage:\n",
        codex_name()
    );
    #[cfg(feature = "headed")]
    {
        s += "  chor [game]                  run a game folder or .game.png (none: the console)\n";
        s += "  chor -t, --tui [game]        play it in the terminal\n";
        s += &format!(
            "  chor -w, --web [dir]         serve a game folder or web build over HTTP\n\
             \x20     -l, --lan                on 0.0.0.0 instead of 127.0.0.1\n\
             \x20     -p, --port <n>           port (default {})\n\
             \x20     -r, --root <web dir>     web build to serve a game folder on\n",
            crate::serve::DEFAULT_PORT
        );
        s += "  chor pack <dir> [out]        pack a folder into a .game.png (alias: bundle)\n";
    }
    #[cfg(not(feature = "headed"))]
    {
        s += "  chor [game]                  play a game folder or .game.png in the terminal\n";
    }
    s += "  chor ignore [dir|-] [-y]     write ignore.lua, the LSP's view of the engine's\n\
          \x20                              commands; in a game it replaces scripts/ignore.lua\n\
          \x20                              after asking (-y: don't ask); - prints it\n";
    s += "  chor -h, --help              this\n";
    s += "  chor -v, --version           engine and codex versions\n";
    s
}

/// `Codex 3.0.0 "Artichoke"`
fn codex_name() -> String {
    crate::asset::get_codex_version_string()
        .trim_start_matches('-')
        .trim()
        .to_string()
}

fn ignore(args: &[String]) -> Result<(), String> {
    let mut dir = None;
    let mut yes = false;
    for a in args {
        match a.as_str() {
            "--yes" | "-y" => yes = true,
            _ if dir.is_none() => dir = Some(a.as_str()),
            _ => return Err(format!("unexpected argument '{}'", a)),
        }
    }
    // Building the VM prints to stdout, which `-` is printing the file to.
    let quiet = Quiet::stdout();
    let docs = crate::command::command_docs();
    drop(quiet);
    let docs = docs.map_err(|e| e.to_string())?;
    let mut docs: Vec<_> = docs.into_iter().collect();
    // Sorted, so the same engine always writes the same file.
    docs.sort_by(|a, b| a.0.cmp(&b.0));
    let fresh = crate::asset::make_codex_file(docs.iter().map(|(k, v)| (k, v)));

    if dir == Some("-") {
        print!("{}", fresh);
        return Ok(());
    }
    let start = PathBuf::from(dir.unwrap_or("."));
    let start = start
        .canonicalize()
        .map_err(|e| format!("{}: {}", start.display(), e))?;
    let target = match game_root(&start) {
        Some(root) => {
            eprintln!("game found at {}", root.display());
            existing_ignore(&root.join("scripts"))
                .unwrap_or_else(|| root.join("scripts").join("ignore.lua"))
        }
        None => start.join("ignore.lua"),
    };

    if let Ok(old) = std::fs::read_to_string(&target) {
        if old == fresh {
            println!("{} is already up to date ({})", target.display(), codex_name());
            return Ok(());
        }
        let new_ver = codex_version(&fresh).unwrap_or_default();
        let note = match codex_version(&old) {
            None => "it has no codex version".to_string(),
            Some(v) if v < new_ver => format!("it's codex {}, older", dotted(v)),
            Some(v) if v > new_ver => format!(
                "it's codex {}, NEWER than this chor's; replacing it goes back",
                dotted(v)
            ),
            Some(_) => "same codex version, but the commands differ".to_string(),
        };
        eprintln!("{} exists: {}", target.display(), note);
        if !yes && !confirm(&format!("replace it with {}?", codex_name()))? {
            println!("left as it was");
            return Ok(());
        }
    }
    std::fs::write(&target, &fresh).map_err(|e| format!("{}: {}", target.display(), e))?;
    println!("wrote {} ({})", target.display(), codex_name());
    Ok(())
}

/// Sends stdout to /dev/null until dropped (where libc is at hand).
struct Quiet(#[cfg(all(unix, feature = "bin"))] Option<i32>);

impl Quiet {
    fn stdout() -> Quiet {
        #[cfg(all(unix, feature = "bin"))]
        unsafe {
            let _ = std::io::stdout().flush();
            let null = libc::open(c"/dev/null".as_ptr(), libc::O_WRONLY);
            let saved = libc::dup(1);
            if null < 0 || saved < 0 {
                return Quiet(None);
            }
            libc::dup2(null, 1);
            libc::close(null);
            Quiet(Some(saved))
        }
        #[cfg(not(all(unix, feature = "bin")))]
        Quiet()
    }
}

impl Drop for Quiet {
    fn drop(&mut self) {
        #[cfg(all(unix, feature = "bin"))]
        if let Some(saved) = self.0 {
            let _ = std::io::stdout().flush();
            unsafe {
                libc::dup2(saved, 1);
                libc::close(saved);
            }
        }
    }
}

/// The game folder `start` is in, if any: the nearest of it and its parents
/// with a `scripts/` folder holding Lua.
fn game_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|d| {
            std::fs::read_dir(d.join("scripts"))
                .map(|mut it| {
                    it.any(|e| {
                        e.map(|e| {
                            e.path()
                                .extension()
                                .and_then(|x| x.to_str())
                                .is_some_and(crate::asset::is_script_ext)
                        })
                            .unwrap_or(false)
                    })
                })
                .unwrap_or(false)
        })
        .map(Path::to_path_buf)
}

/// An `*ignore.lua` already in `scripts`, which the loader skips by name.
fn existing_ignore(scripts: &Path) -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(scripts)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.ends_with("ignore.lua"))
        })
        .collect();
    found.sort();
    found.into_iter().next()
}

/// The `-- Codex x.y.z` in a script's first lines, as the loader reads it.
fn codex_version(script: &str) -> Option<[u16; 3]> {
    let line = script
        .lines()
        .take(5)
        .find(|l| l.starts_with("--") && l.to_lowercase().contains("codex"))?;
    let mut nums = line
        .split(|c: char| !c.is_ascii_digit())
        .filter(|n| !n.is_empty())
        .map(|n| n.parse::<u16>().unwrap_or(0));
    Some([nums.next()?, nums.next().unwrap_or(0), nums.next().unwrap_or(0)])
}

fn dotted(v: [u16; 3]) -> String {
    format!("{}.{}.{}", v[0], v[1], v[2])
}

fn confirm(question: &str) -> Result<bool, String> {
    if !std::io::stdin().is_terminal() {
        return Err(format!("{} (no terminal to ask on; pass -y to replace)", question));
    }
    eprint!("{} [y/N] ", question);
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    std::io::stdin()
        .lock()
        .read_line(&mut answer)
        .map_err(|e| e.to_string())?;
    Ok(matches!(answer.trim().to_lowercase().as_str(), "y" | "yes"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_codex_versions() {
        assert_eq!(codex_version("-- Codex 3.0.0 \"Artichoke\"\nfoo"), Some([3, 0, 0]));
        assert_eq!(codex_version("\n-- codex 2.1\n"), Some([2, 1, 0]));
        assert_eq!(codex_version("print(1)\n"), None);
        assert!(codex_version(&crate::asset::get_codex_version_string()).is_some());
    }
}

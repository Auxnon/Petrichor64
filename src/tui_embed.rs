//! `chor -t <game dir>`: run a game in the terminal with the TUI build of the
//! engine that `tui-embed` (part of `kitchensink`) carries, gzipped. headed and
//! render-tui are separate builds of the core, so this one is a second program:
//! it's unpacked once into the user cache dir, named by a hash of its contents,
//! and then run in place of this process.

include!(concat!(env!("OUT_DIR"), "/tui_embed.rs"));

use std::io::Read;
use std::path::PathBuf;

/// Unpack the TUI build if this chor hasn't already, and return its path.
fn unpacked() -> std::io::Result<PathBuf> {
    let dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("chor");
    let path = dir.join(format!("chor-tui-{}{}", TUI_ID, std::env::consts::EXE_SUFFIX));
    if path.exists() {
        return Ok(path);
    }
    std::fs::create_dir_all(&dir)?;
    let mut data = Vec::new();
    flate2::read::GzDecoder::new(TUI_GZ).read_to_end(&mut data)?;
    // Write beside it and rename, so two chors starting at once never run a
    // half-written binary.
    let tmp = dir.join(format!("chor-tui-{}.{}.tmp", TUI_ID, std::process::id()));
    std::fs::write(&tmp, &data)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }
    std::fs::rename(&tmp, &path)?;
    Ok(path)
}

/// Run the TUI build with `args` (the game dir first, as for chor itself).
/// Only returns on failure; on success the process becomes (or waits for) the
/// TUI and exits with its status.
pub fn run(args: &[String]) -> std::io::Error {
    let path = match unpacked() {
        Ok(p) => p,
        Err(e) => return e,
    };
    let mut cmd = std::process::Command::new(path);
    cmd.args(args);
    #[cfg(unix)]
    {
        // Replace this process, so the terminal and signals go straight to it.
        use std::os::unix::process::CommandExt;
        cmd.exec()
    }
    #[cfg(not(unix))]
    match cmd.status() {
        Ok(status) => std::process::exit(status.code().unwrap_or(1)),
        Err(e) => e,
    }
}

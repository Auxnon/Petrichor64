//! `chor --web <dir>`: a tiny static file server for the web build (e.g.
//! `web/dist`), so testing it needs no trunk, python or node.
//!
//! `<dir>` can also be a game folder (one with `scripts/` or `assets/`). Then
//! the engine comes from a built web dir (`--root`, or found by
//! [`find_web_root`]) and `/game.game.png` is the game, packed fresh on every
//! request, so a browser reload picks up edits with no rebuild.
//!
//! std-only on purpose: no HTTP crate, so the release binary barely grows. It is
//! a dev server, not a production one: GET/HEAD only, one thread per request,
//! `Connection: close`.
//!
//! Every response is sent with COOP/COEP headers, so the `wasm-ultra`
//! (SharedArrayBuffer) build works too, and with `Cache-Control: no-cache`, so
//! the fixed bundle names (filehash=false in Trunk.toml) don't leave the main
//! thread and the VM worker on different builds.

use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

pub const DEFAULT_PORT: u16 = 8080;

/// The game served as `/game.game.png`, repacked on every request.
struct Game {
    dir: PathBuf,
    /// Scratch file pack_folder writes to; the lock keeps two requests from
    /// packing into it at once.
    out: Mutex<PathBuf>,
    rt: tokio::runtime::Runtime,
}

impl Game {
    fn pack(&self) -> Result<Vec<u8>, String> {
        let out = self.out.lock().map_err(|e| e.to_string())?;
        let mut loggy = crate::log::Loggy::new();
        self.rt
            .block_on(crate::asset::pack_folder(
                &self.dir.to_string_lossy(),
                Some(&out.to_string_lossy()),
                &mut loggy,
            ))
            .map_err(|e| e.to_string())?;
        std::fs::read(&*out).map_err(|e| e.to_string())
    }
}

/// A game folder, as opposed to a web dir to serve as-is.
fn is_game_dir(dir: &Path) -> bool {
    !dir.join("index.html").is_file()
        && (dir.join("scripts").is_dir() || dir.join("assets").is_dir())
}

/// Where a built web dir is likely to be: `web/dist` under the working
/// directory, `web/` beside the executable (a shipped layout), or the repo's
/// `web/dist` when running from `target/<profile>/`.
fn find_web_root() -> Option<PathBuf> {
    let mut candidates = vec![PathBuf::from("web/dist")];
    if let Some(exe_dir) = std::env::current_exe().ok().and_then(|e| e.parent().map(Path::to_path_buf)) {
        candidates.push(exe_dir.join("web"));
        candidates.push(exe_dir.join("../../web/dist"));
    }
    candidates.into_iter().find(|c| c.join("index.html").is_file())
}

/// Serve `path` until the process is killed: a web dir as-is, or a game folder
/// on top of the web dir `root` (searched for when None). `lan` binds 0.0.0.0
/// instead of 127.0.0.1, so other devices on the network can reach it.
pub fn run(path: &str, root: Option<&str>, port: u16, lan: bool) -> Result<(), String> {
    let path = std::fs::canonicalize(path).map_err(|e| format!("'{}': {}", path, e))?;
    if !path.is_dir() {
        return Err(format!("'{}' is not a directory", path.display()));
    }
    let (root, game) = if is_game_dir(&path) || root.is_some() {
        let root = match root {
            Some(r) => PathBuf::from(r),
            None => find_web_root().ok_or(
                "no built web dir found for this game (looked for web/dist here, and \
                 web/ beside chor); build one with `just web-build` or pass --root <dir>",
            )?,
        };
        let root = std::fs::canonicalize(&root).map_err(|e| format!("'{}': {}", root.display(), e))?;
        let out = std::env::temp_dir().join(format!("chor-web-{}.game.png", port));
        let rt = tokio::runtime::Runtime::new().map_err(|e| e.to_string())?;
        let game = Game { dir: path, out: Mutex::new(out), rt };
        // Pack once up front so a broken game fails here, not in the browser.
        game.pack()?;
        (root, Some(std::sync::Arc::new(game)))
    } else {
        (path, None)
    };
    let host = if lan {
        Ipv4Addr::UNSPECIFIED
    } else {
        Ipv4Addr::LOCALHOST
    };
    let listener = TcpListener::bind(SocketAddr::from((host, port)))
        .map_err(|e| format!("can't bind {}:{}: {}", host, port, e))?;

    println!("serving {}", root.display());
    if let Some(g) = &game {
        println!("game    {}  (repacked on each reload)", g.dir.display());
    }
    println!("  http://localhost:{}/", port);
    if lan {
        match lan_ip() {
            Some(ip) => println!("  http://{}:{}/  (lan)", ip, port),
            None => println!("  http://<this machine's ip>:{}/  (lan)", port),
        }
    }
    println!("ctrl+c to stop");

    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let root = root.clone();
        let game = game.clone();
        std::thread::spawn(move || {
            if let Err(e) = handle(stream, &root, game.as_deref()) {
                ::log::debug!("serve: {}", e);
            }
        });
    }
    Ok(())
}

/// The address this machine uses to reach the network. Connecting a UDP socket
/// sends nothing; it only makes the OS pick the outbound interface.
fn lan_ip() -> Option<std::net::IpAddr> {
    let s = UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("10.255.255.255:1").ok()?;
    let ip = s.local_addr().ok()?.ip();
    (!ip.is_unspecified() && !ip.is_loopback()).then_some(ip)
}

fn handle(mut stream: TcpStream, root: &Path, game: Option<&Game>) -> std::io::Result<()> {
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line)?;
    // Drain the headers; nothing in them changes the response.
    let mut line = String::new();
    while reader.read_line(&mut line)? > 2 {
        line.clear();
    }

    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("/");
    let head = method == "HEAD";
    if method != "GET" && !head {
        return respond(&mut stream, "405 Method Not Allowed", "text/plain", b"method not allowed", head);
    }

    if let Some(game) = game {
        if target.split(['?', '#']).next() == Some("/game.game.png") {
            return match game.pack() {
                Ok(body) => respond(&mut stream, "200 OK", "image/png", &body, head),
                Err(e) => {
                    eprintln!("pack failed: {}", e);
                    respond(&mut stream, "500 Internal Server Error", "text/plain", e.as_bytes(), head)
                }
            };
        }
    }

    let Some(mut path) = resolve(root, target) else {
        return respond(&mut stream, "403 Forbidden", "text/plain", b"forbidden", head);
    };
    if path.is_dir() {
        path.push("index.html");
    }
    match std::fs::read(&path) {
        Ok(body) => respond(&mut stream, "200 OK", mime(&path), &body, head),
        // A real 404, not an index.html fallback: the engine probes
        // /game.game.png and falls back to its embedded bundle when it's missing.
        Err(_) => respond(&mut stream, "404 Not Found", "text/plain", b"not found", head),
    }
}

/// Map a request target onto a file under `root`. None if it would escape it.
fn resolve(root: &Path, target: &str) -> Option<PathBuf> {
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let decoded = percent_decode(path)?;
    let mut out = root.to_path_buf();
    for c in Path::new(decoded.trim_start_matches('/')).components() {
        match c {
            Component::Normal(p) => out.push(p),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(out)
}

fn percent_decode(s: &str) -> Option<String> {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' {
            let hex = std::str::from_utf8(b.get(i + 1..i + 3)?).ok()?;
            out.push(u8::from_str_radix(hex, 16).ok()?);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        // Must be exact for WebAssembly.instantiateStreaming.
        "wasm" => "application/wasm",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "ogg" => "audio/ogg",
        "wav" => "audio/wav",
        "glb" => "model/gltf-binary",
        "gltf" => "model/gltf+json",
        "lua" | "txt" | "md" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

fn respond(
    stream: &mut TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
    head: bool,
) -> std::io::Result<()> {
    let header = format!(
        "HTTP/1.1 {status}\r\n\
         Content-Type: {content_type}\r\n\
         Content-Length: {}\r\n\
         Cache-Control: no-cache\r\n\
         Cross-Origin-Opener-Policy: same-origin\r\n\
         Cross-Origin-Embedder-Policy: require-corp\r\n\
         Connection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    if !head {
        stream.write_all(body)?;
    }
    stream.flush()
}

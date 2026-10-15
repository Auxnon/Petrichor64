//! `chor --web <dir>`: a tiny static file server for the web build (e.g.
//! `web/dist`), so testing it needs no trunk, python or node.
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

pub const DEFAULT_PORT: u16 = 8080;

/// Serve `root` until the process is killed. `lan` binds 0.0.0.0 instead of
/// 127.0.0.1, so other devices on the network can reach it.
pub fn run(root: &str, port: u16, lan: bool) -> Result<(), String> {
    let root = std::fs::canonicalize(root).map_err(|e| format!("'{}': {}", root, e))?;
    if !root.is_dir() {
        return Err(format!("'{}' is not a directory", root.display()));
    }
    let host = if lan {
        Ipv4Addr::UNSPECIFIED
    } else {
        Ipv4Addr::LOCALHOST
    };
    let listener = TcpListener::bind(SocketAddr::from((host, port)))
        .map_err(|e| format!("can't bind {}:{}: {}", host, port, e))?;

    println!("serving {}", root.display());
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
        std::thread::spawn(move || {
            if let Err(e) = handle(stream, &root) {
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

fn handle(mut stream: TcpStream, root: &Path) -> std::io::Result<()> {
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

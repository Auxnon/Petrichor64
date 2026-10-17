//! Keep the engine's own printing off the terminal picture.
//!
//! The engine logs with plain `println!`/`eprintln!` from everywhere (Loggy,
//! loaders, Lua errors), and the TUI draws its frames on that same stdout. A
//! Lua `loop()` that throws prints three lines every frame, which used to land
//! mid-frame and scroll the screen: garbled and jittering. So while the TUI
//! runs, fds 1 and 2 point at a pipe, a thread collects what comes through,
//! and frames go to a duplicate of the real terminal instead. The renderer
//! shows the recent lines in a strip under the picture, and whatever was
//! logged last is printed again once the terminal is restored.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::fd::{FromRawFd, RawFd};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// How long a line stays in the strip after it was last printed. Boot noise
/// clears on its own; an error repeated every frame stays up.
const SHOW_FOR: Duration = Duration::from_secs(6);
/// Lines kept for the strip and the after-exit dump.
const KEEP: usize = 40;

pub struct Line {
    pub text: String,
    pub count: u32,
    pub seen: Instant,
}

#[derive(Clone, Default)]
pub struct LogTail(Arc<Mutex<VecDeque<Line>>>);

impl LogTail {
    fn push(&self, text: String) {
        let Ok(mut lines) = self.0.lock() else { return };
        let now = Instant::now();
        // A line seen recently (a per-frame error is several lines that
        // alternate) is counted instead of repeated.
        if let Some(l) = lines.iter_mut().rev().take(8).find(|l| l.text == text) {
            l.count += 1;
            l.seen = now;
            return;
        }
        if lines.len() >= KEEP {
            lines.pop_front();
        }
        lines.push_back(Line { text, count: 1, seen: now });
    }

    /// The most recent `max` lines printed within `SHOW_FOR`, oldest first.
    pub fn recent(&self, max: usize) -> Vec<String> {
        let Ok(lines) = self.0.lock() else { return vec![] };
        let now = Instant::now();
        let mut out: Vec<String> = lines
            .iter()
            .rev()
            .filter(|l| now.duration_since(l.seen) < SHOW_FOR)
            .take(max)
            .map(label)
            .collect();
        out.reverse();
        out
    }

    fn all(&self) -> Vec<String> {
        let Ok(lines) = self.0.lock() else { return vec![] };
        lines.iter().map(label).collect()
    }
}

fn label(l: &Line) -> String {
    if l.count > 1 {
        format!("{} (x{})", l.text, l.count)
    } else {
        l.text.clone()
    }
}

/// While alive, stdout and stderr go to `tail`; `term` is the real terminal.
pub struct Capture {
    pub term: File,
    pub tail: LogTail,
    saved_out: RawFd,
    saved_err: RawFd,
}

impl Capture {
    /// None if the fds can't be juggled; the caller then draws on stdout as
    /// before.
    pub fn start() -> Option<Capture> {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        // SAFETY: plain fd syscalls on fds this process owns; every result is
        // checked before use.
        unsafe {
            let saved_out = libc::dup(1);
            let saved_err = libc::dup(2);
            let term_fd = libc::dup(1);
            let mut pipe = [0; 2];
            if saved_out < 0 || saved_err < 0 || term_fd < 0 || libc::pipe(pipe.as_mut_ptr()) != 0 {
                return None;
            }
            if libc::dup2(pipe[1], 1) < 0 || libc::dup2(pipe[1], 2) < 0 {
                libc::dup2(saved_out, 1);
                libc::dup2(saved_err, 2);
                return None;
            }
            libc::close(pipe[1]);
            let tail = LogTail::default();
            let reader = File::from_raw_fd(pipe[0]);
            let sink = tail.clone();
            std::thread::spawn(move || {
                for line in BufReader::new(reader).lines() {
                    let Ok(line) = line else { break };
                    let line = line.trim_end().to_string();
                    if !line.is_empty() {
                        sink.push(line);
                    }
                }
            });
            Some(Capture {
                term: File::from_raw_fd(term_fd),
                tail,
                saved_out,
                saved_err,
            })
        }
    }

    pub fn saved_err(&self) -> RawFd {
        self.saved_err
    }

    /// Point stdout/stderr back at the terminal (after leaving the alternate
    /// screen) and print the lines logged last, so an error is still readable
    /// after quitting.
    pub fn finish(self) {
        use std::io::Write;
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().flush();
        // SAFETY: restoring the fds saved in `start`.
        unsafe {
            libc::dup2(self.saved_out, 1);
            libc::dup2(self.saved_err, 2);
            libc::close(self.saved_out);
            libc::close(self.saved_err);
        }
        // Only worth repeating if something went wrong; boot chatter isn't.
        let lines = self.tail.all();
        let trouble = lines.iter().any(|l| {
            let l = l.to_lowercase();
            ["error", "panic", "cannot", "missing", "failed"]
                .iter()
                .any(|w| l.contains(w))
        });
        if trouble {
            let mut err = std::io::stderr();
            let _ = writeln!(err, "-- last log lines --");
            for l in lines.iter().skip(lines.len().saturating_sub(20)) {
                let _ = writeln!(err, "{}", l);
            }
        }
    }
}

/// After a panic the terminal is still raw, on the alternate screen, with
/// stderr in the pipe; this puts it back first so the panic message shows.
pub fn restore_on_panic(saved_err: RawFd) {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        // SAFETY: only fd and terminal-mode calls on our own fds.
        unsafe {
            libc::dup2(saved_err, 1);
            libc::dup2(saved_err, 2);
        }
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stderr(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        previous(info);
    }));
}

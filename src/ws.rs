//! WebSocket client transport behind `conn("ws://…")` (the `ws` feature).
//!
//! Both targets expose the same poll-shaped `WsLink`: `send` queues a text
//! frame, `recv` pops one if any arrived, `status` reports a failure or close.
//! Nothing here blocks the Lua thread.
//!
//! - **native**: a dedicated thread owns a blocking `tungstenite` socket. Its
//!   TCP read timeout is short, so the thread alternates between draining
//!   inbound frames and flushing the outbound channel.
//! - **wasm**: Lua runs in the VM web worker, which has the browser's own
//!   `WebSocket`. That handle is `!Send` and silt userdata must be `Send`, so
//!   the socket lives in a worker-local registry and `WsLink` only carries its
//!   id. The worker is single threaded, so the registry is never contended.
//!
//! Frames sent before the socket opens are queued, not dropped, so a script can
//! `conn` and immediately `send` its handshake.

#[cfg(not(target_arch = "wasm32"))]
pub use native::WsLink;
#[cfg(target_arch = "wasm32")]
pub use web::WsLink;

/// Whether `addr` names a WebSocket endpoint. A leading `/` is a path relative
/// to the page origin, which only resolves on the web.
pub fn is_ws_addr(addr: &str) -> bool {
    addr.starts_with("ws://") || addr.starts_with("wss://") || addr.starts_with('/')
}

#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::{
        io::ErrorKind,
        net::TcpStream,
        sync::{
            mpsc::{channel, Receiver, Sender, TryRecvError},
            Arc,
        },
        time::Duration,
    };

    use parking_lot::Mutex;
    use tungstenite::{stream::MaybeTlsStream, Message, WebSocket};

    /// How long a read may block before the thread checks for outbound frames.
    const POLL: Duration = Duration::from_millis(8);

    enum Out {
        Text(String),
        Close,
    }

    pub struct WsLink {
        out: Sender<Out>,
        inbox: Receiver<String>,
        /// `Some` once the thread has ended: the reason, or "safe close".
        status: Arc<Mutex<Option<String>>>,
    }

    impl WsLink {
        pub fn open(url: &str) -> Result<Self, String> {
            if url.starts_with('/') {
                return Err(format!(
                    "relative websocket path '{}' only resolves on the web, use ws://host{}",
                    url, url
                ));
            }
            let (out_tx, out_rx) = channel();
            let (in_tx, in_rx) = channel();
            let status = Arc::new(Mutex::new(None));
            let st = status.clone();
            let url = url.to_owned();
            std::thread::spawn(move || {
                let end = match run(&url, out_rx, in_tx) {
                    Ok(()) => "safe close".to_owned(),
                    Err(e) => e,
                };
                *st.lock() = Some(end);
            });
            Ok(WsLink {
                out: out_tx,
                inbox: in_rx,
                status,
            })
        }

        pub fn send(&self, text: String) {
            let _ = self.out.send(Out::Text(text));
        }

        pub fn recv(&self) -> Option<String> {
            self.inbox.try_recv().ok()
        }

        pub fn status(&self) -> Option<String> {
            self.status.lock().clone()
        }

        pub fn close(&self) {
            let _ = self.out.send(Out::Close);
        }
    }

    impl Drop for WsLink {
        fn drop(&mut self) {
            self.close();
        }
    }

    fn tcp(sock: &WebSocket<MaybeTlsStream<TcpStream>>) -> Option<&TcpStream> {
        match sock.get_ref() {
            MaybeTlsStream::Plain(s) => Some(s),
            MaybeTlsStream::Rustls(s) => Some(s.get_ref()),
            _ => None,
        }
    }

    fn run(url: &str, out: Receiver<Out>, inbox: Sender<String>) -> Result<(), String> {
        let (mut sock, _) = tungstenite::connect(url).map_err(|e| e.to_string())?;
        if let Some(s) = tcp(&sock) {
            s.set_read_timeout(Some(POLL)).map_err(|e| e.to_string())?;
        }
        loop {
            loop {
                match out.try_recv() {
                    Ok(Out::Text(t)) => {
                        sock.send(Message::text(t)).map_err(|e| e.to_string())?
                    }
                    Ok(Out::Close) | Err(TryRecvError::Disconnected) => {
                        let _ = sock.close(None);
                        let _ = sock.flush();
                        return Ok(());
                    }
                    Err(TryRecvError::Empty) => break,
                }
            }
            match sock.read() {
                Ok(Message::Text(t)) => {
                    if inbox.send(t.as_str().to_owned()).is_err() {
                        return Ok(());
                    }
                }
                Ok(Message::Binary(b)) => {
                    if inbox
                        .send(String::from_utf8_lossy(&b).into_owned())
                        .is_err()
                    {
                        return Ok(());
                    }
                }
                Ok(Message::Close(_)) => return Ok(()),
                Ok(_) => {}
                Err(tungstenite::Error::Io(e))
                    if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(tungstenite::Error::ConnectionClosed) => return Ok(()),
                Err(e) => return Err(e.to_string()),
            }
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod web {
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};

    use rustc_hash::FxHashMap;
    use wasm_bindgen::{prelude::Closure, JsCast};
    use web_sys::{CloseEvent, Event, MessageEvent, WebSocket};

    struct Shared {
        open: bool,
        pending: Vec<String>,
        inbox: VecDeque<String>,
        status: Option<String>,
    }

    struct Entry {
        sock: WebSocket,
        shared: Rc<RefCell<Shared>>,
        // Kept alive for as long as the socket can fire them.
        _handlers: [Closure<dyn FnMut(Event)>; 2],
        _onmessage: Closure<dyn FnMut(MessageEvent)>,
        _onclose: Closure<dyn FnMut(CloseEvent)>,
    }

    thread_local! {
        static SOCKETS: RefCell<FxHashMap<u32, Entry>> = RefCell::new(FxHashMap::default());
        static NEXT_ID: RefCell<u32> = const { RefCell::new(1) };
    }

    /// Turn a page-relative path into a ws(s) URL on the worker's own origin.
    fn resolve(url: &str) -> Result<String, String> {
        if !url.starts_with('/') {
            return Ok(url.to_owned());
        }
        let scope: web_sys::WorkerGlobalScope = js_sys::global()
            .dyn_into()
            .map_err(|_| "relative websocket path needs a worker scope".to_owned())?;
        let loc = scope.location();
        let proto = if loc.protocol() == "https:" { "wss:" } else { "ws:" };
        Ok(format!("{}//{}{}", proto, loc.host(), url))
    }

    pub struct WsLink {
        id: u32,
    }

    impl WsLink {
        pub fn open(url: &str) -> Result<Self, String> {
            let url = resolve(url)?;
            let sock = WebSocket::new(&url).map_err(|e| format!("{:?}", e))?;
            let shared = Rc::new(RefCell::new(Shared {
                open: false,
                pending: vec![],
                inbox: VecDeque::new(),
                status: None,
            }));

            let (s, sk) = (shared.clone(), sock.clone());
            let onopen = Closure::<dyn FnMut(Event)>::new(move |_| {
                let mut sh = s.borrow_mut();
                sh.open = true;
                for t in sh.pending.drain(..) {
                    let _ = sk.send_with_str(&t);
                }
            });
            let s = shared.clone();
            let onerror = Closure::<dyn FnMut(Event)>::new(move |_| {
                let mut sh = s.borrow_mut();
                if sh.status.is_none() {
                    sh.status = Some("websocket error".to_owned());
                }
            });
            let s = shared.clone();
            let onmessage = Closure::<dyn FnMut(MessageEvent)>::new(move |e: MessageEvent| {
                if let Some(t) = e.data().as_string() {
                    s.borrow_mut().inbox.push_back(t);
                }
            });
            let s = shared.clone();
            let onclose = Closure::<dyn FnMut(CloseEvent)>::new(move |e: CloseEvent| {
                let mut sh = s.borrow_mut();
                sh.open = false;
                if sh.status.is_none() {
                    sh.status = Some(if e.was_clean() {
                        "safe close".to_owned()
                    } else {
                        format!("closed ({})", e.code())
                    });
                }
            });
            sock.set_onopen(Some(onopen.as_ref().unchecked_ref()));
            sock.set_onerror(Some(onerror.as_ref().unchecked_ref()));
            sock.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
            sock.set_onclose(Some(onclose.as_ref().unchecked_ref()));

            let id = NEXT_ID.with(|n| {
                let mut n = n.borrow_mut();
                let id = *n;
                *n += 1;
                id
            });
            SOCKETS.with(|m| {
                m.borrow_mut().insert(
                    id,
                    Entry {
                        sock,
                        shared,
                        _handlers: [onopen, onerror],
                        _onmessage: onmessage,
                        _onclose: onclose,
                    },
                )
            });
            Ok(WsLink { id })
        }

        fn with<R>(&self, f: impl FnOnce(&Entry) -> R) -> Option<R> {
            SOCKETS.with(|m| m.borrow().get(&self.id).map(f))
        }

        pub fn send(&self, text: String) {
            self.with(|e| {
                let mut sh = e.shared.borrow_mut();
                if sh.open {
                    let _ = e.sock.send_with_str(&text);
                } else if sh.status.is_none() {
                    sh.pending.push(text);
                }
            });
        }

        pub fn recv(&self) -> Option<String> {
            self.with(|e| e.shared.borrow_mut().inbox.pop_front())
                .flatten()
        }

        pub fn status(&self) -> Option<String> {
            self.with(|e| e.shared.borrow().status.clone())
                .unwrap_or_else(|| Some("safe close".to_owned()))
        }

        pub fn close(&self) {
            if let Some(e) = SOCKETS.with(|m| m.borrow_mut().remove(&self.id)) {
                e.sock.set_onopen(None);
                e.sock.set_onerror(None);
                e.sock.set_onmessage(None);
                e.sock.set_onclose(None);
                let _ = e.sock.close();
            }
        }
    }

    impl Drop for WsLink {
        fn drop(&mut self) {
            self.close();
        }
    }
}

//! The `connection` userdata returned by `conn`. One Lua type over two
//! transports, chosen by address: `ws://`, `wss://` or a page-relative `/path`
//! is a WebSocket (`ws` feature, works on the web), anything else is the raw
//! TCP + MessagePack link (`online_capable`, native only, can host).
//!
//! Userdata types are registered by name, so both transports must share this
//! one type rather than each defining its own "connection".

#[cfg(feature = "online_capable")]
use std::sync::mpsc::{Receiver, Sender};

use silt_lua::{
    prelude::Value,
    userdata::{UserData, UserDataMethods},
    LuaError,
};

#[cfg(feature = "online_capable")]
use crate::packet::{Packer, Packet64};

#[cfg(feature = "online_capable")]
type Handle = std::thread::JoinHandle<Result<(), String>>;

#[cfg(feature = "online_capable")]
pub struct TcpLink {
    sender: Sender<Packet64>,
    reciever: Receiver<Packet64>,
    handle: Option<Handle>,
    closed: bool,
    id: u16,
}

#[cfg(feature = "online_capable")]
impl TcpLink {
    pub fn new(sender: Sender<Packet64>, reciever: Receiver<Packet64>, handle: Handle) -> Self {
        TcpLink {
            sender,
            reciever,
            handle: Some(handle),
            closed: false,
            id: 0,
        }
    }

    fn send(&mut self, s: String) {
        if self.sender.send(Packet64::str(self.id, 0, s)).is_err() {
            self.shutdown();
        }
    }

    fn recv(&mut self) -> Option<String> {
        if let Ok(r) = self.reciever.try_recv() {
            if let Packer::Str(s) = r.body() {
                return Some(s.to_owned());
            }
        }
        None
    }

    fn status(&mut self) -> Option<String> {
        if self.closed {
            return Some("closed".to_owned());
        }
        let finished = match &self.handle {
            Some(h) => h.is_finished(),
            None => return Some("closed".to_owned()),
        };
        if finished {
            self.closed = true;
            let h = self.handle.take().unwrap();
            return Some(match h.join() {
                Err(_) => "join failed".to_string(),
                Ok(Ok(_)) => "safe close".to_string(),
                Ok(Err(e)) => e,
            });
        }
        None
    }

    fn shutdown(&mut self) {
        self.closed = true;
        if let Some(h) = self.handle.take() {
            if let Err(e) = self.sender.send(Packet64::close()) {
                println!("failed to send close: {}", e);
            }
            let m = match h.join() {
                Ok(Ok(_)) => "safe close".to_owned(),
                Ok(Err(e)) => e,
                Err(_) => "join failed".to_owned(),
            };
            println!("connection shutdown {}", m);
        }
    }
}

pub enum Link {
    #[cfg(feature = "online_capable")]
    Tcp(TcpLink),
    #[cfg(feature = "ws")]
    Ws(crate::ws::WsLink),
}

pub struct LuaConnection {
    link: Link,
}

impl LuaConnection {
    pub fn new(link: Link) -> Self {
        LuaConnection { link }
    }

    /// Open by address; see the module docs for how the transport is picked.
    pub fn open(addr: &str, udp: bool, server: bool) -> Result<Self, String> {
        #[cfg(feature = "ws")]
        if crate::ws::is_ws_addr(addr) {
            return crate::ws::WsLink::open(addr).map(|l| Self::new(Link::Ws(l)));
        }
        #[cfg(feature = "online_capable")]
        {
            return crate::online::Online::open(addr, udp, server)
                .map(|l| Self::new(Link::Tcp(l)))
                .map_err(|e| e.to_string());
        }
        #[allow(unreachable_code)]
        {
            let _ = (udp, server);
            Err(format!(
                "no transport for '{}' in this build (ws:// needs the `ws` feature, tcp needs `online_capable`)",
                addr
            ))
        }
    }

    fn send(&mut self, s: String) {
        match &mut self.link {
            #[cfg(feature = "online_capable")]
            Link::Tcp(l) => l.send(s),
            #[cfg(feature = "ws")]
            Link::Ws(l) => l.send(s),
        }
    }

    fn recv(&mut self) -> Option<String> {
        match &mut self.link {
            #[cfg(feature = "online_capable")]
            Link::Tcp(l) => l.recv(),
            #[cfg(feature = "ws")]
            Link::Ws(l) => l.recv(),
        }
    }

    fn status(&mut self) -> Option<String> {
        match &mut self.link {
            #[cfg(feature = "online_capable")]
            Link::Tcp(l) => l.status(),
            #[cfg(feature = "ws")]
            Link::Ws(l) => l.status(),
        }
    }

    fn shutdown(&mut self) {
        match &mut self.link {
            #[cfg(feature = "online_capable")]
            Link::Tcp(l) => l.shutdown(),
            #[cfg(feature = "ws")]
            Link::Ws(l) => l.close(),
        }
    }
}

impl UserData for LuaConnection {
    fn get_id(&self) -> usize {
        4
    }
    fn type_name() -> &'static str {
        "connection"
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        // A string goes out verbatim; a table is encoded as json first.
        methods.add_method_mut("send", |_, _, this, v: Option<Value>| {
            let Some(conn) = this else {
                return Err(LuaError::UDBadCall);
            };
            let text = match v {
                Some(Value::String(s)) => s.to_string(),
                Some(t @ Value::Table(_)) => crate::json::to_json(&t)
                    .map_err(LuaError::Custom)?
                    .to_string(),
                Some(other) => other.to_string(),
                None => return Ok(()),
            };
            conn.send(text);
            Ok(())
        });
        // Next message or nil. `recv(true)` decodes it from json; a message
        // that isn't valid json comes back as its raw string.
        methods.add_method_mut("recv", |vm, mc, this, decode: Option<bool>| {
            let Some(conn) = this else {
                return Err(LuaError::UDBadCall);
            };
            Ok(match conn.recv() {
                None => Value::Nil,
                Some(s) if decode.unwrap_or(false) => {
                    match serde_json::from_str::<serde_json::Value>(&s) {
                        Ok(j) => crate::json::from_json(vm, mc, j),
                        Err(_) => vm.new_string(mc, s),
                    }
                }
                Some(s) => vm.new_string(mc, s),
            })
        });
        methods.add_method_mut("test", |vm, mc, this, (): ()| {
            Ok(this.and_then(|c| c.status()).map(|s| vm.new_string(mc, s)))
        });
        methods.add_method_mut("kill", |_, _, this, (): ()| {
            match this {
                Some(conn) => conn.shutdown(),
                None => return Err(LuaError::UDBadCall),
            };
            Ok(())
        });
    }
}

impl Drop for LuaConnection {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl ToString for LuaConnection {
    fn to_string(&self) -> String {
        match &self.link {
            #[cfg(feature = "online_capable")]
            Link::Tcp(l) => format!("connection(tcp {})", l.id),
            #[cfg(feature = "ws")]
            Link::Ws(_) => "connection(ws)".to_owned(),
        }
    }
}

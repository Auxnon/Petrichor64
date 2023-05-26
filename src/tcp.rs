// a rust tokio chat client

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures::future::{self, Either};
use futures::sync::mpsc;
use futures::{Future, Sink, Stream};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::prelude::*;
use tokio::timer::Delay;

use crate::chat::Chat;
use crate::client::Client;
use crate::client::ClientMessage;
use crate::client::ServerMessage;
use crate::client::ServerMessage::*;

mod chat;
mod client;

fn main() {
    let addr = "0.0.0.0:8080".parse().unwrap();
    let listener = TcpListener::bind(&addr).unwrap();

    let chat = Arc::new(Chat::new());

    let server = listener.incoming().for_each(move |socket| {
        let chat = chat.clone();
        let (reader, writer) = socket.split();
        let client = Client::new(reader, writer);
        let (client_reader, client_writer) = client.split();
        let (tx, rx) = mpsc::unbounded();
        let client = client_reader
            .map(ClientMessage::Message)
            .select(rx.map(ClientMessage::Command))
            .forward(client_writer.sink_map_err(|_| ()))
            .map(|_| ())
            .map_err(|_| ());
        tokio::spawn(client);
        chat.add_client(tx);
        Ok(())
    });

    println!("Listening on: {}", addr);
    tokio::run(server);
}

// Path: src/chat.rs
// a rust tokio chat client

use std::collections::HashMap;
use std::sync::Arc;

use futures::sync::mpsc;
use futures::{Future, Sink, Stream};
use tokio::prelude::*;

use crate::client::ClientMessage;
use crate::client::ServerMessage;
use crate::client::ServerMessage::*;

pub struct Chat {
    clients: HashMap<String, mpsc::UnboundedSender<ServerMessage>>,
}

impl Chat {
    pub fn new() -> Self {
        Chat {
            clients: HashMap::new(),
        }
    }

    pub fn add_client(&mut self, client: mpsc::UnboundedSender<ServerMessage>) {
        self.clients.insert("".to_string(), client);
    }
}

// Path: src/client.rs
// a rust tokio chat client

use std::io;
use std::time::Duration;

use futures::future::{self, Either};
use futures::sync::mpsc;
use futures::{Future, Sink, Stream};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::prelude::*;
use tokio::timer::Delay;

use crate::chat::Chat;
use crate::client::Client;
use crate::client::ClientMessage;
use crate::client::ServerMessage;
use crate::client::ServerMessage::*;

pub enum ClientMessage {
    Message(String),
    Command(String),
}

pub enum ServerMessage {
    Message(String),
    Joined(String),
    Left(String),
}

pub struct Client<R, W> {
    reader: R,
    writer: W,
}

impl<R, W> Client<R, W>
where
    R: AsyncRead + Send + 'static,
    W: AsyncWrite + Send + 'static,
{
    pub fn new(reader: R, writer: W) -> Self {
        Client { reader, writer }
    }

    pub fn split(self) -> (ClientReader<R>, ClientWriter<W>) {
        let Client { reader, writer } = self;
        let reader = ClientReader { inner: reader };
        let writer = ClientWriter { inner: writer };
        (reader, writer)
    }
}

pub struct ClientReader<R> {
    inner: R,
}

impl<R> Stream for ClientReader<R>
where
    R: AsyncRead + Send + 'static,
{
    type Item = String;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        let mut buf = vec![0; 1024];
        match self.inner.poll_read(&mut buf) {
            Ok(Async::Ready(n)) => {
                if n == 0 {
                    return Ok(Async::Ready(None));
                }
                let s = String::from_utf8_lossy(&buf[..n]).into_owned();
                Ok(Async::Ready(Some(s)))
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => Err(e),
        }
    }
}

pub struct ClientWriter<W> {
    inner: W,
}

impl<W> Sink for ClientWriter<W>
where
    W: AsyncWrite + Send + 'static,
{
    type SinkItem = String;
    type SinkError = io::Error;

    fn start_send(&mut self, item: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let buf = item.into_bytes();
        match self.inner.poll_write(&buf) {
            Ok(Async::Ready(n)) => {
                if n == 0 {
                    return Ok(AsyncSink::NotReady(item));
                }
                Ok(AsyncSink::Ready)
            }
            Ok(Async::NotReady) => Ok(AsyncSink::NotReady(item)),
            Err(e) => Err(e),
        }
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.inner.poll_flush()
    }
}

// Path: src/main.rs
// a rust tokio chat client

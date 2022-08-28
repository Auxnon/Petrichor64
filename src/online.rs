// use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};

use std::{
    error::Error,
    net::SocketAddr,
    sync::mpsc::{channel, Receiver, Sender},
};

pub type MovePacket = Vec<f32>;
pub type MoveReponse = f32;

#[tokio::main]
pub async fn init() -> Result<(Sender<MovePacket>, Receiver<MovePacket>), Box<dyn Error>> {
    let tcp = true;

    // Parse what address we're going to connect to
    let addr = "127.0.0.1:6142";
    let addr = addr.parse::<SocketAddr>()?;
    // world_sender:

    let (netin_send, netin_recv) = channel::<MovePacket>();
    let (netout_send, netout_recv) = channel::<MovePacket>();

    // let netin:Sender<MovePacket>=
    // let netout=
    // let stdin = FramedRead::new(io::stdin(), BytesCodec::new());
    // let stdin = stdin.map(|i| i.map(|bytes| bytes.freeze()));
    // let stdout = FramedWrite::new(io::stdout(), BytesCodec::new());

    // if tcp {
    //     tcp::connect(&addr, stdin, stdout).await?;
    // } else {
    udp::connect(&addr, netin_send, netout_recv).await?;
    // }
    Ok((netout_send, netin_recv))
}

mod tcp {
    use bytes::Bytes;
    use futures::{future, Sink, SinkExt, Stream, StreamExt};
    use std::{error::Error, io, net::SocketAddr};
    use tokio::net::TcpStream;
    use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};

    pub async fn connect(
        addr: &SocketAddr,
        mut stdin: impl Stream<Item = Result<Bytes, io::Error>> + Unpin,
        mut stdout: impl Sink<Bytes, Error = io::Error> + Unpin,
    ) -> Result<(), Box<dyn Error>> {
        let mut stream = TcpStream::connect(addr).await?;
        let (r, w) = stream.split();
        let mut sink = FramedWrite::new(w, BytesCodec::new());
        // filter map Result<BytesMut, Error> stream into just a Bytes stream to match stdout Sink
        // on the event of an Error, log the error and end the stream
        let mut stream = FramedRead::new(r, BytesCodec::new())
            .filter_map(|i| match i {
                //BytesMut into Bytes
                Ok(i) => future::ready(Some(i.freeze())),
                Err(e) => {
                    println!("failed to read from socket; error={}", e);
                    future::ready(None)
                }
            })
            .map(Ok);

        match future::join(sink.send_all(&mut stdin), stdout.send_all(&mut stream)).await {
            (Err(e), _) | (_, Err(e)) => Err(e.into()),
            _ => Ok(()),
        }
    }
}

mod udp {
    use bytes::Bytes;
    use futures::{Sink, SinkExt, Stream, StreamExt};
    use std::error::Error;
    use std::io;
    use std::net::SocketAddr;
    use std::sync::mpsc::{Receiver, Sender};
    use tokio::net::UdpSocket;

    use super::MovePacket;

    pub async fn connect(
        addr: &SocketAddr,
        // stdin: impl Stream<Item = Result<Bytes, io::Error>> + Unpin,
        // stdout: impl Sink<Bytes, Error = io::Error> + Unpin,
        netin: Sender<MovePacket>,
        netout: Receiver<MovePacket>,
    ) -> Result<(), Box<dyn Error>> {
        // We'll bind our UDP socket to a local IP/port, but for now we
        // basically let the OS pick both of those.
        let bind_addr = if addr.ip().is_ipv4() {
            "0.0.0.0:0"
        } else {
            "[::]:0"
        };

        let socket = UdpSocket::bind(&bind_addr).await?;
        socket.connect(addr).await?;

        tokio::try_join!(send(netout, &socket), recv(netin, &socket))?;

        Ok(())
    }

    async fn send(
        // mut stdin: impl Stream<Item = Result<Bytes, io::Error>> + Unpin,
        netout: Receiver<MovePacket>,
        writer: &UdpSocket,
    ) -> Result<(), io::Error> {
        // while let Some(item) = stdin.next().await {
        //     let buf = item?;
        //     writer.send(&buf[..]).await?;
        // }
        loop {
            match netout.recv() {
                Ok(p) => {
                    let p2 = p.iter().map(|f| (*f * 100.) as u8).collect::<Vec<u8>>();
                    writer.send(&p2).await?;
                }
                Err(e) => {
                    return Err(io::Error::new(io::ErrorKind::Interrupted, e));
                }
            }
        }

        Ok(())
    }

    async fn recv(
        // mut stdout: impl Sink<Bytes, Error = io::Error> + Unpin,
        netin: Sender<MovePacket>,
        reader: &UdpSocket,
    ) -> Result<(), io::Error> {
        loop {
            let mut buf = vec![0; 1024];
            let n = reader.recv(&mut buf[..]).await?;

            if n > 0 {
                // let b = Bytes::from(buf);
                netin.send(buf.iter().map(|x| (*x as f32) / 100.).collect());

                // match{
                //     std::sync::mpsc::SendError(e) => {
                //         return Err(e);
                //     }
                //     _ => {}
                // };
                // stdout.send(b).await?;
            }
        }
        Ok(())
    }
}

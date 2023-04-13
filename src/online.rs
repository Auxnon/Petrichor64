// use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};

use std::{
    error::Error,
    net::SocketAddr,
    sync::mpsc::{channel, Receiver, Sender},
    thread::JoinHandle,
};

use futures::{FutureExt, Stream};
use tokio::{
    io::{self, AsyncWriteExt},
    net::{TcpListener, TcpSocket, TcpStream, UdpSocket},
    sync::mpsc,
};

use crate::lua_connection::LuaConnection;

pub type MovePacket = Vec<f32>;
pub type MessagePacket = String;
pub type MoveReponse = f32;

pub enum Packet64 {
    U3([f32; 3]),
    U4([f32; 4]),
    Str(String),
}

trait Connection {}
pub struct Online {
    clients: Vec<JoinHandle<String>>,
}
impl Online {
    pub fn new() -> Self {
        Online { clients: vec![] }
    }

    // pub fn init() -> Result<(Sender<MovePacket>, Receiver<MovePacket>), Box<dyn Error>> {
    //     let tcp = true;

    //     // let netin:Sender<MovePacket>=
    //     // let netout=
    //     // let stdin = FramedRead::new(io::stdin(), BytesCodec::new());
    //     // let stdin = stdin.map(|i| i.map(|bytes| bytes.freeze()));
    //     // let stdout = FramedWrite::new(io::stdout(), BytesCodec::new());

    //     // if tcp {
    //     //     tcp::connect(&addr, stdin, stdout).await?;
    //     // } else {
    //     // crate::lg!("online starting..");

    //     // crate::lg!("online on");
    // }

    /** address in format 0.0.0.0:1234 */
    pub fn open(
        &mut self,
        address: &str,
        udp: bool,
        server: bool,
    ) -> Result<LuaConnection, Box<dyn Error>> {
        // let addr = "73.101.41.242:6142";
        let addr = address.parse::<SocketAddr>()?;

        let (netin_send, netin_recv) = channel::<Packet64>();
        let (netout_send, netout_recv) = channel::<Packet64>();
        // MARK we must poll this somehow
        let handle = std::thread::spawn(move || {
            // futures::executor::block_on(
            run(addr, udp, false, netin_send, netout_recv);
            "done".to_string()
        });

        self.clients.push(handle);
        let conn = LuaConnection::new(netout_send, netin_recv);
        Ok(conn)
    }
    pub fn shutdown(&mut self) {
        for client in self.clients.drain(..) {
            client.thread().unpark();
            let res = client.join();
            println!("terminated connection: {:?}", res);
        }
    }
}

#[tokio::main(flavor = "multi_thread")]
async fn run(
    addr: SocketAddr,
    udp: bool,
    server: bool,
    netin: Sender<Packet64>,
    netout: Receiver<Packet64>,
) -> Result<(), Box<dyn Error>> {
    //impl futures::Future<Output = Result<(), Box<(dyn Error)>>> {
    if !udp {
        let socket = create_tcp_client(addr).await?;
        let re = tcp::connect(socket, netin, netout).await;
        re
    } else {
        let socket = create_udp_client(addr).await?;
        let re = udp::connect(socket, netin, netout).await;
        // println!("🟣🟢🔴 terminated socket");
        re
    }
}

async fn create_udp_client(addr: SocketAddr) -> io::Result<UdpSocket> {
    let bind_addr = if addr.ip().is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };

    let socket = UdpSocket::bind(bind_addr).await?;
    socket.connect(addr).await?;
    return Ok(socket);
}

// async fn create_udp_server(addr: SocketAddr) -> io::Result<UdpSocket> {
//     let bind_addr = if addr.ip().is_ipv4() {
//         "0.0.0.0:0"
//     } else {
//         "[::]:0"
//     };
// }

async fn create_tcp_client(addr: SocketAddr) -> io::Result<TcpStream> {
    let bind_addr = if addr.ip().is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };
    let handle = 1;
    let socket = TcpStream::connect(addr).await?;
    // type Tx = mpsc::UnboundedSender<Bytes>;
    // let socket = TcpListener::bind(bind_addr).await?;
    // socket.
    return Ok(socket);
}
type Rx = mpsc::UnboundedReceiver<String>;
type Tx = mpsc::UnboundedSender<String>;

mod tcp {

    use futures::channel::mpsc;
    use std::convert::TryInto;
    use std::error::Error;
    use std::io::{self};
    use std::sync::mpsc::{Receiver, Sender};
    use std::sync::Arc;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
    use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
    use tokio::net::TcpStream;
    use tokio_util::codec;

    use super::{MessagePacket, MovePacket, Packet64};

    pub struct Handshake {
        pub name: String,
    }

    impl Handshake {
        pub fn new<S: Into<String>>(name: S) -> Handshake {
            Handshake { name: name.into() }
        }
    }

    // pub type HandshakeCodec = codec::LengthPrefixedJson<Handshake, Handshake>;

    pub async fn start_server(socket: TcpStream) {}

    pub async fn connect(
        socket: TcpStream,
        netin: Sender<Packet64>,
        netout: Receiver<Packet64>,
    ) -> Result<(), Box<dyn Error>> {
        let (read, write) = socket.into_split();
        let reader = BufReader::new(read);
        let writer = BufWriter::new(write);

        let task1 = tokio::spawn(send(reader, netin));
        let task2 = tokio::spawn(recv(writer, netout));

        tokio::try_join!(
            async move {
                match task1.await {
                    Err(e) => {
                        println!("task1 erroring");
                        Err(std::io::Error::new(std::io::ErrorKind::Other, "closing!"))
                    }
                    Ok(o) => match o {
                        Ok(m) => Ok(()),
                        Err(e) => Err(e),
                    },
                }
            },
            async move {
                match task2.await {
                    Err(e) => {
                        println!("task2 erroring");
                        Err(std::io::Error::new(std::io::ErrorKind::Other, "closing!"))
                    }
                    Ok(o) => match o {
                        Ok(m) => Ok(()),
                        Err(e) => Err(e),
                    },
                }
            }
        )?;
        Ok(())
    }

    /** send content to the bundle from outside */
    pub async fn send(
        mut socket: BufReader<OwnedReadHalf>,
        netin: Sender<Packet64>,
    ) -> Result<(), io::Error> {
        loop {
            let mut out_buf = Default::default();
            if let Ok(b) = socket.read_line(&mut out_buf).await {
                // if b == 0 {
                //     break;
                // }
                netin.send(Packet64::Str(out_buf));
            }
        }
        Ok(())
    }

    pub async fn recv(
        mut socket: BufWriter<OwnedWriteHalf>,
        netout: Receiver<Packet64>,
    ) -> Result<(), io::Error> {
        loop {
            let cmd = netout.recv();
            if let Ok(v) = cmd {
                // socket.send(&b).await?;
                if let Packet64::Str(src) = v {
                    socket.write_all(src.as_bytes()).await?;
                }
                // socket.write_buf(src)
                // socket.write_all(&v).await?;
            }
        }

        Ok(())
    }
}

mod udp {
    use std::convert::TryInto;
    use std::error::Error;
    use std::io;
    use std::sync::mpsc::{Receiver, Sender};
    use std::sync::Arc;
    use tokio::net::UdpSocket;

    use super::{MovePacket, Packet64};

    pub async fn connect(
        socket: UdpSocket,
        netin: Sender<Packet64>,
        netout: Receiver<Packet64>,
    ) -> Result<(), Box<dyn Error>> {
        let sock1 = Arc::new(socket);
        let sock2 = sock1.clone();

        let task1 = tokio::spawn(send(sock1, netin));
        let task2 = tokio::spawn(recv(sock2, netout));

        tokio::try_join!(
            async move {
                match task1.await {
                    Err(e) => {
                        println!("task1 erroring");
                        Err(std::io::Error::new(std::io::ErrorKind::Other, "closing!"))
                    }
                    Ok(o) => match o {
                        Ok(m) => Ok(()),
                        Err(e) => Err(e),
                    },
                }
            },
            async move {
                match task2.await {
                    Err(e) => {
                        println!("task2 erroring");
                        Err(std::io::Error::new(std::io::ErrorKind::Other, "closing!"))
                    }
                    Ok(o) => match o {
                        Ok(m) => Ok(()),
                        Err(e) => Err(e),
                    },
                }
            }
        )?;
        Ok(())
    }

    pub async fn send(socket: Arc<UdpSocket>, netin: Sender<Packet64>) -> Result<(), io::Error> {
        loop {
            let mut out_buf = [0u8; 16];

            match socket.recv(&mut out_buf[..]).await {
                Ok(b) => {
                    let p = crate::online::tof32(&out_buf);
                    println!("🟣 netin recv {:?}", p);
                    netin.send(Packet64::U4(p));
                }

                Err(e) => {
                    println!("🟣🔴 recv err:{}", e);
                }
            }
        }
        Ok(())
    }

    pub async fn recv(socket: Arc<UdpSocket>, netout: Receiver<Packet64>) -> Result<(), io::Error> {
        loop {
            let cmd = netout.recv();
            if let Ok(v) = cmd {
                if let Packet64::U4(f) = v {
                    let b = crate::online::fromf32(&f);
                    socket.send(&b).await?;
                }
                // if (v[0] == -99.) {
                //     let e = std::io::Error::new(std::io::ErrorKind::Other, "closing!");
                //     return Err(e);
                // }
                // if v.len() < 4 {
                //     v.resize(4, 0.);
                // }
                // v.split(pred)
                // let f: [f32; 4] = v.try_into().unwrap();
                // let r = v.try_into();
                // match r {
                //     Ok(i) => {
            }
        }

        Ok(())
    }

    // fn create(netin: Sender<MovePacket>, netout: Receiver<MovePacket>, socket: UdpSocket) {
    //     tokio::task::spawn(run(netin, netout, socket));
    // }

    //#[tokio::main]
    // async fn run(
    //     netin: Sender<MovePacket>,
    //     netout: Receiver<MovePacket>,
    //     socket: UdpSocket,
    // ) -> Result<(), io::Error> {
    //     tokio::runtime::Builder::new_multi_thread()
    //         .enable_all()
    //         .build()
    //         .unwrap()
    //         .block_on(async {
    //             println!("Hello world");

    //             loop {
    //                 let to_send = match netout.try_recv() {
    //                     Ok(p) => {
    //                         println!("yes we a lua send in");
    //                         Some(p)
    //                     }
    //                     _ => {
    //                         println!("loop, no lua in packet");
    //                         None
    //                     }
    //                 };

    //                 //MARK confirmed that this works, so the mpsc channel is just closing to early, like the sender is being dropped somehow???
    //                 let to_send = Some(vec![0., 3., 4.]);

    //                 if let Some(p) = to_send {
    //                     let b = p.iter().map(|f| (*f * 100.) as u8).collect::<Vec<u8>>();
    //                     let amt = socket.send(&b).await?;
    //                     println!("eched back {:?}", std::thread::current().id());
    //                 }
    //                 let mut out_buf = [0u8; 3];

    //                 match socket.try_recv(&mut out_buf) {
    //                     Ok(b) => {
    //                         let p = out_buf
    //                             .iter()
    //                             .map(|u| (*u as f32) / 100.)
    //                             .collect::<Vec<f32>>();
    //                         netin.send(p);
    //                     }
    //                     _ => {}
    //                 }

    //                 match socket.try_recv_from(&mut out_buf) {
    //                     Ok((n, _addr)) => {
    //                         println!("GOT {:?}", &out_buf[..3]);
    //                         // break;
    //                     }
    //                     Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    //                         continue;
    //                     }
    //                     Err(e) => return Err(e),
    //                 };
    //                 println!("read some stuff {:?}", std::thread::current().id());
    //             }
    //         })
    // }
}

pub fn fromf32(data: &[f32; 4]) -> [u8; 16] {
    let mut res = [0; 16];
    for i in 0..4 {
        res[4 * i..][..4].copy_from_slice(&data[i].to_le_bytes());
    }
    res
}

pub fn tof32(data: &[u8; 16]) -> [f32; 4] {
    let mut res = [0f32; 4];
    for i in 0..4 {
        let mut f = [0u8; 4];
        f.copy_from_slice(&data[4 * i..][..4]);
        res[i] = f32::from_le_bytes(f);
    }
    res
}

// pub fn convertf32(data: &[f32; 4]) -> [u8; 16] {
//     let mut res = [0; 16];
//     for i in 0..4 {
//         res[4 * i..][..4].copy_from_slice(&data[i].to_le_bytes());
//     }
//     res
// }

// pub fn cf32(data: &[f32; 4]) {
//     for i in 0..4 {
//         data[i].to_le_bytes()
//     }
// }

// fn test(f: [f32; 4]) {
//     // let buf = Vec::with_capacity(1024).writer();
//     f[0].to_be_bytes()

//     // byteorder::
//     assert_eq!(1024, buf.get_ref().capacity());
// }

// fn to_byte_slice<'a>(floats: &'a [f32]) -> &'a [u8] {
//     // bytemuck::cast()
//     unsafe { std::slice::from_raw_parts(floats.as_ptr() as *const _, floats.len() * 4) }
// }

// fn to_float_vec(b: [u8]) {
//     let f = unsafe { b.align_to::<f32>() };
// }

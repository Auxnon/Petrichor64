use std::{
    error::Error,
    net::SocketAddr,
    sync::mpsc::{channel, Receiver, Sender},
};

use tokio::{
    io::{self},
    net::{TcpStream, UdpSocket},
    sync::mpsc::{self},
};

use crate::{lua_connection::LuaConnection, packet::Packet64};

// packets over tcp
// 1 byte for type
// 4 bytes for length
// 64 bytes for data
// 1 byte for checksum
// 1 byte for end

trait Connection {}
pub struct Online {}
impl Online {
    /** address in format 0.0.0.0:1234 */
    pub fn open(address: &str, udp: bool, server: bool) -> Result<LuaConnection, Box<dyn Error>> {
        let addr = address.parse::<SocketAddr>()?;

        let (netin_send, netin_recv) = channel::<Packet64>();
        let (netout_send, netout_recv) = channel::<Packet64>();
        // MARK we must poll this somehow
        let handle =
            std::thread::spawn(
                move || match run(addr, udp, server, netin_send, netout_recv) {
                    Ok(_) => Ok(()),
                    Err(e) => Err(e.to_string()),
                },
            );

        // println!("opening connection to {}", address);
        let conn = LuaConnection::new(netout_send, netin_recv, handle);
        Ok(conn)
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
    if !server {
        tcp::start_client(addr, netin, netout).await
        // let socket = create_tcp_client(addr).await?;
        // let re = tcp::connect(socket, netin, netout).await;
    } else {
        tcp::start_server(addr, netin, netout).await
    }

    // else {
    //     let socket = create_udp_client(addr).await?;
    //     let re = udp::connect(socket, netin, netout).await;
    //     re
    // }
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
    // let bind_addr = if addr.ip().is_ipv4() {
    //     "0.0.0.0:0"
    // } else {
    //     "[::]:0"
    // };
    let socket = TcpStream::connect(addr).await?;
    return Ok(socket);
}
// type Rx = mpsc::UnboundedReceiver<String>;
// type Tx = mpsc::UnboundedSender<String>;

mod tcp {
    use super::Packet64;
    use crate::packet::{Packer, Packy, WrappedSink, WrappedStream};
    use crate::parse::test;
    use futures::sink::SinkExt;
    use futures::StreamExt;
    use parking_lot::Mutex;
    use rustc_hash::FxHashMap;
    use std::error::Error;
    use std::io::{self};
    use std::net::SocketAddr;
    use std::sync::mpsc::{channel, Receiver, Sender};
    use std::sync::Arc;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
    use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
    use tokio::net::{TcpListener, TcpStream};
    use tokio_util::codec::{Framed, FramedRead, FramedWrite};

    // type Streamy = Framed<WrappedStream, BasicPacky>;
    // type DeSink = Framed<WrappedSink, (), MyMessage, Json<(), MyMessage>>;

    pub async fn start_client(
        addr: SocketAddr,
        netin: Sender<Packet64>,
        netout: Receiver<Packet64>,
    ) -> Result<(), Box<dyn Error>> {
        let socket = TcpStream::connect(addr).await?;
        connect(None, socket, netin, netout).await
    }

    pub async fn connect(
        id: Option<u16>,
        socket: TcpStream,
        netin: Sender<Packet64>,
        netout: Receiver<Packet64>,
    ) -> Result<(), Box<dyn Error>> {
        let (read, write) = socket.into_split();

        let reader = WrappedStream::new(read, Packy {});
        let writer = WrappedSink::new(write, Packy {});

        let task1 = tokio::spawn(recv(reader, netin));
        let task2 = tokio::spawn(send(writer, netout));

        tokio::try_join!(
            async move {
                match task1.await {
                    Err(_) => Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "receiver failed start",
                    )),
                    Ok(o) => match o {
                        Err(e) => Err(e),
                        _ => Ok(()),
                    },
                }
            },
            async move {
                match task2.await {
                    Err(_) => Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        "sender failed start",
                    )),
                    Ok(o) => match o {
                        Err(e) => Err(e),
                        _ => Ok(()),
                    },
                }
            }
        )?;
        Ok(())
    }

    pub async fn server_checker(
        netout: Receiver<Packet64>,
        vec_out: Arc<Mutex<FxHashMap<u16, Sender<Packet64>>>>,
    ) -> Result<(), String> {
        loop {
            println!("server checker loop");
            if let Ok(packet) = netout.recv() {
                // broadcaster.send(packet);
                let t = packet.target();
                let b = packet.body();
                if let Packer::Close() = b {
                    println!("close");
                    return Ok(());
                }
                if *t == 0 {
                    println!("broadcast to all {:?}", packet);
                    for (_, conn) in vec_out.lock().iter() {
                        conn.send(packet.clone());
                    }
                } else {
                    println!("broadcast to {} {:?}", *t, packet);
                    for (id, conn) in vec_out.lock().iter() {
                        if id == t {
                            conn.send(packet.clone());
                        }
                    }
                }
            }
            // if let Ok((id, conn)) = new_conn.try_recv() {
            //     connect(id, conn, netin, netout)
            // }
            // for conn in conn_in.iter() {
            //     conn.send(packet.clone());
            // }
        }
    }

    pub async fn start_server(
        addr: SocketAddr,
        netin: Sender<Packet64>,
        netout: Receiver<Packet64>,
    ) -> Result<(), Box<dyn Error>> {
        let net_listener = TcpListener::bind(&addr).await?;
        println!("Listening on: {}", addr);
        let mut id_counter = 1;
        // let (serv_in, serv_out) = channel::<(u16, TcpStream)>(100);
        let vec_in = Arc::new(Mutex::new(FxHashMap::default()));
        let v2 = vec_in.clone();
        // let (ssend, srecv) = tokio::sync::mpsc::channel::<Packet64>(100);
        // let mut connections: Vec<(u16, tokio::sync::mpsc::Sender<Packet64>)> = vec![];
        // let (maker, hearer) = tokio::sync::broadcast::channel::<TcpStream>(10);

        // server_checker(netout, broadcaster).await
        // let spreader = tokio::spawn(async move {
        //     loop {
        //         if let Ok(packet) = netout.try_recv() {
        //             broadcaster.send(packet);
        //         }
        //         if let Ok(new_conn)= hearer.try_recv() {
        //             connect(id, conn, netin, netout)
        //         }
        //     }
        // });
        // let connections = FxHashMap::default();

        tokio::spawn(async move { server_checker(netout, v2).await });

        loop {
            // Asynchronously wait for an inbound socket.
            // net_listener.poll_accept(cx: &mut Context<'_>)
            // net_listener.
            let (socket, _) = net_listener.accept().await?;
            println!("Got connection from: {}", socket.peer_addr()?);
            // let (read, mut write) = socket.into_split();
            let id = id_counter;
            id_counter += 1;

            let (conn_in, conn_out) = channel::<Packet64>();
            vec_in.lock().insert(id, conn_in);
            // serv_in.send(((id, socket)));
            // connect::<tokio::sync::broadcast::Receiver<Packet64>>(Some(id), socket, netin);
            let netnew = netin.clone();
            tokio::spawn(async move {
                connect(Some(id), socket, netnew, conn_out).await;
                // let rt = tokio::runtime::Runtime::new().unwrap();
                // rt.block_on(async move { connect(Some(id), socket, netnew, conn_out).await })
                //     .unwrap();
            });

            // let (read, write) = socket.into_split();
            // let mut writer = WrappedSink::new(write, Packy {});
            // let reader = WrappedStream::new(read, Packy {});

            // let mut reader = BufReader::new(read);
            // let mut bytes: Vec<u8> = Vec::new();
            // tokio::spawn(async move {
            //     //     // let mut buf: String = String::new();

            //     //     // In a loop, read data from the socket and write the data back.
            //     loop {
            //         if let Ok(m) = conn_out.try_recv() {
            //             writer.send(m).await;
            //         }
            //         reader.
            //         //         // let n = socket
            //         //         //     .read(&mut buf)
            //         //         //     .await
            //         //         //     .expect("failed to read data from socket");
            //         //         bytes.clear();
            //         //         let n = reader.read_until(b'\r', &mut bytes).await.unwrap();
            //         //         // bytes.pop();
            //         //         println!("read {} bytes", n);
            //         //         if n == 0 {
            //         //             return;
            //         //         }
            //         //         if let Ok(s) = std::str::from_utf8(&bytes) {
            //         //             println!("read data: {:?}", &s[0..n - 1]);
            //         //             write
            //         //                 .write_u64(id)
            //         //                 .await
            //         //                 .expect("failed to write id to socket");
            //         //             write
            //         //                 .write_all(&bytes)
            //         //                 .await
            //         //                 .expect("failed to write data to socket");
            //         //             write.flush().await.expect("failed to flush data to socket");
            //         //         }
            //     }
            // });
        }
    }

    /** send content to the bundle from outside */
    pub async fn recv(mut socket: WrappedStream, netin: Sender<Packet64>) -> Result<(), io::Error> {
        loop {
            // println!("recv loop");
            if let Some(a) = socket.next().await {
                match a {
                    Ok(p) => {
                        println!("in_buf: {:?}", p.body());
                        if let Err(_) = netin.send(p) {
                            return Err(io::Error::new(
                                io::ErrorKind::BrokenPipe,
                                "lua recv pipe fail",
                            ));
                        }
                    }
                    Err(e) => {
                        println!("recv error: {:?}", e);
                        return Err(io::Error::new(
                            io::ErrorKind::BrokenPipe,
                            "server pipe close",
                        ));
                    }
                }
            }
        }
    }

    pub async fn send(
        mut socket: WrappedSink,
        mut netout: Receiver<Packet64>,
    ) -> Result<(), io::Error> {
        // let sink = Framed::new(socket, Packet64 {});
        loop {
            println!("send loop");
            let cmd = netout.recv();
            if let Ok(p) = cmd {
                println!("out_buf: {:?}", p.body());
                match p.body() {
                    Packer::Str(src) => {
                        println!("out_buf: {:?}", src);
                        // TODO built in flush, may be inefficient with multiple packets, batch somehow?
                        socket.send(p).await?;
                        // socket.write_all(src.as_bytes()).await?;
                        // const R: u32 = 0x0d;
                        // socket.write_u32(R).await?;
                        // socket.flush().await?;
                    }
                    Packer::Close() => {
                        println!("socket shutdown");
                        return Ok(());
                    }
                    _ => {}
                }
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "server pipe close",
                ));
            }
        }
    }
}

mod udp {
    use std::error::Error;
    use std::io;
    use std::sync::mpsc::{Receiver, Sender};
    use std::sync::Arc;
    use tokio::net::UdpSocket;

    use super::Packet64;

    pub async fn connect(
        socket: UdpSocket,
        netin: Sender<Packet64>,
        netout: Receiver<Packet64>,
    ) -> Result<(), Box<dyn Error>> {
        let sock1 = Arc::new(socket);
        let sock2 = sock1.clone();

        // let task1 = tokio::spawn(recv(sock1, netin));
        // let task2 = tokio::spawn(send(sock2, netout));

        // tokio::try_join!(
        //     async move {
        //         match task1.await {
        //             Err(e) => {
        //                 println!("task1 erroring");
        //                 Err(std::io::Error::new(std::io::ErrorKind::Other, "closing!"))
        //             }
        //             Ok(o) => match o {
        //                 Ok(m) => Ok(()),
        //                 Err(e) => Err(e),
        //             },
        //         }
        //     },
        //     async move {
        //         match task2.await {
        //             Err(e) => {
        //                 println!("task2 erroring");
        //                 Err(std::io::Error::new(std::io::ErrorKind::Other, "closing!"))
        //             }
        //             Ok(o) => match o {
        //                 Ok(m) => Ok(()),
        //                 Err(e) => Err(e),
        //             },
        //         }
        //     }
        // )?;
        Ok(())
    }

    // pub async fn recv(socket: Arc<UdpSocket>, netin: Sender<Packet64>) -> Result<(), io::Error> {
    //     loop {
    //         let mut out_buf = [0u8; 16];

    //         match socket.recv(&mut out_buf[..]).await {
    //             Ok(b) => {
    //                 let p = crate::online::tof32(&out_buf);
    //                 println!("🟣 netin recv {:?}", p);
    //                 netin.send(Packet64::U4(p));
    //             }

    //             Err(e) => {
    //                 println!("🟣🔴 recv err:{}", e);
    //             }
    //         }
    //     }
    // }

    // pub async fn send(socket: Arc<UdpSocket>, netout: Receiver<Packet64>) -> Result<(), io::Error> {
    //     loop {
    //         let cmd = netout.recv();
    //         if let Ok(v) = cmd {
    //             match v {
    //                 Packet64::U4(f) => {
    //                     let b = crate::online::fromf32(&f);
    //                     socket.send(&b).await?;
    //                 }
    //                 Packet64::Close() => {
    //                     println!("got shutdown packet");
    //                     return Ok(());
    //                 }
    //                 _ => {}
    //             }
    //         }
    //     }
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

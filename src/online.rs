// use tokio_util::codec::{BytesCodec, FramedRead, FramedWrite};

use std::{
    error::Error,
    net::SocketAddr,
    sync::mpsc::{channel, Receiver, Sender},
};

use futures::{FutureExt, Stream};
use tokio::{
    io::{self, AsyncWriteExt},
    net::UdpSocket,
    sync::mpsc,
};

pub type MovePacket = Vec<f32>;
pub type MoveReponse = f32;

pub fn init() -> Result<
    (
        Sender<MovePacket>,
        Receiver<MovePacket>, // ,Result<(), Box<dyn Error>>, //impl futures::Future<Output = Result<(), Box<dyn Error>>>,
    ),
    Box<dyn Error>,
> {
    let tcp = true;

    // Parse what address we're going to connect to
    // let addr = "127.0.0.1:6142"; //6142
    // let addr = "https://makeavoy.com:6142";
    let addr = "73.101.41.242:6142";
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
    crate::lg!("online starting..");
    // MARK we must poll this somehow
    let h = std::thread::spawn(move || {
        // futures::executor::block_on(

        run(addr, netin_send, netout_recv);
    });

    // make_instance();
    crate::lg!("online on");
    Ok((netout_send, netin_recv))
}

#[tokio::main(flavor = "multi_thread")]
async fn run(
    addr: SocketAddr,
    netin: Sender<MovePacket>,
    netout: Receiver<MovePacket>,
) -> Result<(), Box<dyn Error>> {
    //impl futures::Future<Output = Result<(), Box<(dyn Error)>>> {
    let socket = create_socket(addr).await;
    // let socket2 = create_socket(addr).await;
    let re = udp::connect(socket, netin, netout).await;
    println!("🟣🟢🔴 terminated socket");
    re
}

async fn create_socket(addr: SocketAddr) -> UdpSocket {
    let bind_addr = if addr.ip().is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    };

    let socket = match UdpSocket::bind(bind_addr).await {
        Err(e) => {
            eprintln!("🟣🟢🔴 sock_bind_err {}", e);
            panic!("hi")
        }
        Ok(s) => {
            println!("🟣🟢 sock made");
            s
        }
    };

    match socket.connect(addr).await {
        Err(e) => {
            eprintln!("🟣🟢🔴 sock_con_err {}", e);
        }
        _ => {
            println!("🟣🟢 sock connected {}", addr);
        }
    }

    // let (mut socket_sink, socket_stream) = UdpFramed::new(socket, BytesCodec::new()).split();
    // socket.set_reuse_address(true)
    return socket;
}

mod udp {
    use futures::{FutureExt, Sink, SinkExt, Stream, StreamExt};
    use std::convert::TryInto;
    use std::error::Error;
    use std::sync::mpsc::{Receiver, Sender};
    use std::sync::Arc;
    use std::{io, thread};
    use tokio::net::UdpSocket;

    use crate::lg;

    use super::MovePacket;

    pub async fn connect(
        // addr: SocketAddr,
        socket: UdpSocket,
        // socket2: UdpSocket,
        // stdin: impl Stream<Item = Result<Bytes, io::Error>> + Unpin,
        // stdout: impl Sink<Bytes, Error = io::Error> + Unpin,
        netin: Sender<MovePacket>,
        netout: Receiver<MovePacket>,
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
        // manager.abort()

        lg!("online looping complete");

        Ok(())
    }

    pub async fn send(socket: Arc<UdpSocket>, netin: Sender<MovePacket>) -> Result<(), io::Error> {
        // println!("🟣 entry");

        loop {
            // println!("🟣 in start");

            let mut out_buf = [0u8; 16];

            match socket.recv(&mut out_buf[..]).await {
                Ok(b) => {
                    // let p = out_buf
                    //     .iter()
                    //     .map(|u| (*u as f32) / 100.)
                    //     .collect::<Vec<f32>>();
                    let p = crate::online::tof32(&out_buf);
                    println!("🟣 netin recv {:?}", p);
                    netin.send(p.to_vec());
                }
                // Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
                //     continue;
                // }
                Err(e) => {
                    println!("🟣🔴 recv err:{}", e);
                    // break;
                }
            }

            // println!("🟣 in end");
        }
        Ok(())
    }

    pub async fn recv(
        socket: Arc<UdpSocket>,
        netout: Receiver<MovePacket>,
    ) -> Result<(), io::Error> {
        // println!("🟢 entry");
        loop {
            // println!("🟢 out start");
            let cmd = netout.recv();
            match cmd {
                Ok(mut v) => {
                    if (v[0] == -99.) {
                        let e = std::io::Error::new(std::io::ErrorKind::Other, "closing!");
                        // println!("🟢 trigger close");
                        return Err(e);
                    }
                    if v.len() < 4 {
                        v.resize(4, 0.);
                    }
                    // v.split(pred)
                    let f: [f32; 4] = v.try_into().unwrap();
                    // let r = v.try_into();
                    // match r {
                    //     Ok(i) => {
                    let b = crate::online::fromf32(&f);
                    socket.send(&b).await?;
                    //     }
                    //     Err(_) => {}
                    // }

                    // let b = crate::online::fromf32(&.unwrap());
                    // let b = v.iter().map(|f| (*f * 100.) as u8).collect::<Vec<u8>>();
                    // println!("🟢 netout recv {:?}", b);
                }
                Err(e) => {
                    // eprintln!("🟢🔴 err_recv: {}", e);
                    // break;
                    // retur
                }
            }
            // println!("🟢 out end");
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

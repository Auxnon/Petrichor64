use mlua::{UserData, UserDataMethods};
use std::sync::mpsc::{Receiver, Sender};

use crate::packet::{Packer, Packet64};

type Handle = std::thread::JoinHandle<Result<(), String>>;

pub struct LuaConnection {
    sender: Sender<Packet64>,
    reciever: Receiver<Packet64>,
    handle: Option<Handle>,
    closed: bool,
    server: bool,
    // peers: Vec<u64>,
    id: u16,
}

impl LuaConnection {
    pub fn new(sender: Sender<Packet64>, reciever: Receiver<Packet64>, handle: Handle) -> Self {
        LuaConnection {
            sender,
            reciever,
            handle: Some(handle),
            closed: false,
            server: false,

            id: 0,
        }
    }
    pub fn shutdown(&mut self) {
        self.closed = true;
        if let Some(h) = self.handle.take() {
            if let Err(e) = self.sender.send(Packet64::close()) {
                println!("failed to send close: {}", e);
            }
            let m = match h.join().unwrap() {
                Ok(_) => "safe close".to_owned(),
                Err(e) => e.to_owned(),
            };
            println!("connection shutdown {}", m);
        } else {
            println!("connection already shutdown");
        }
    }
}

impl UserData for LuaConnection {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method_mut("send", |_, this, s: String| {
            if let Err(_) = this.sender.send(Packet64::str(this.id, 0, s)) {
                this.shutdown();
            }
            Ok(())
        });
        methods.add_method_mut("recv", |_, this, (): ()| {
            if let Ok(r) = this.reciever.try_recv() {
                if let Packer::Str(s) = r.body() {
                    return Ok(Some(s.to_owned()));
                }
            }
            Ok(None)
        });
        methods.add_method_mut("test", |_, this, (): ()| {
            if this.closed {
                return Ok(Some("closed".to_owned()));
            }
            let finished = match &this.handle {
                Some(h) => h.is_finished(),
                None => return Ok(Some("closed".to_owned())),
            };

            if finished {
                this.closed = true;
                let h = this.handle.take().unwrap();
                let message = match h.join() {
                    Err(_) => "join failed".to_string(),
                    Ok(r) => match r {
                        Ok(_) => "safe close".to_string(),
                        Err(e) => e.to_owned(),
                    },
                };
                return Ok(Some(message));
            }
            Ok(None)
        });

        methods.add_method_mut("kill", |_, this, (): ()| {
            this.shutdown();
            Ok(())
        });
    }
}

impl Drop for LuaConnection {
    fn drop(&mut self) {
        // println!("dropping connection);
        self.shutdown();
    }
}
// impl serde::Serialize for LuaEnt {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer {
//         todo!()
//     }
// }

// lua!(
//     "send",
//     move |_, (x, y, z): (f32, f32, f32)| {
//         #[cfg(feature = "online_capable")]
//         {
//             match &netout {
//                 Some(nout) => {
//                     // crate::lg!("send from com {},{}", x, y);
//                     match nout.send(vec![x, y, z]) {
//                         Ok(d) => {}
//                         Err(e) => {
//                             // println!("damn we got {}", e);
//                         }
//                     }
//                 }
//                 _ => {
//                     // crate::lg!("ain't got no online");
//                 }
//             }
//         }
//         Ok(())
//     },
//     "Send UDP",
//     "-- Coming soon"
// );

// lua!(
//     "recv",
//     move |_, _: ()| {
//         #[cfg(feature = "online_capable")]
//         {
//             match &netin {
//                 Some(nin) => {
//                     match nin.try_recv() {
//                         Ok(r) => {
//                             return Ok(r);
//                             // crate::lg!("udp {:?}", r);
//                         }
//                         _ => {}
//                     }
//                 }
//                 _ => {
//                     // crate::lg!("ain't got no online");
//                 }
//             }
//         }
//         Ok(vec![0., 0., 0.])
//     },
//     "Recieve UDP",
//     "-- Coming soon"
// );

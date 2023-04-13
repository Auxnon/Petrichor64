use mlua::UserData;
use std::sync::mpsc::{Receiver, Sender};

use crate::online::Packet64;

pub struct LuaConnection {
    sender: Sender<Packet64>,
    reciever: Receiver<Packet64>,
}

impl LuaConnection {
    pub fn new(sender: Sender<Packet64>, reciever: Receiver<Packet64>) -> Self {
        LuaConnection { sender, reciever }
    }
}
impl UserData for LuaConnection {}

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

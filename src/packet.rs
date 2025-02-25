use bytes::{Buf, BytesMut};
use itertools::{izip, Itertools};
use rmp_serde::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};
use std::{error::Error, iter::zip};
use tokio::net::{
    tcp::{OwnedReadHalf, OwnedWriteHalf},
    TcpStream,
};
use tokio_util::codec::{Decoder, Encoder, FramedRead, FramedWrite};

#[cfg(feature = "puc_lua")]
use mlua::{Table, Value};
#[cfg(feature = "silt")]
use silt_lua::prelude::{Table, Value};

const MAX_BYTES: usize = 8 * 1024 * 1024;

// #[derive(Serialize, Deserialize, Debug)]
// pub struct BasicPacky {
//     target: u16,
//     id: u16,
//     style: u8,
//     message: String,
// }

pub type WrappedStream = FramedRead<OwnedReadHalf, Packy>;
pub type WrappedSink = FramedWrite<OwnedWriteHalf, Packy>;
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Packet64 {
    id: u16,
    target: u16,
    body: Packer,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Packer {
    Str(String),
    FVec(Vec<f32>),
    UVec(Vec<u32>),
    // Connection(u16, Box<WrappedSink>),
    Close(),
}
impl Packet64 {
    pub fn new(id: u16, target: u16, body: Packer) -> Self {
        Packet64 { id, target, body }
    }
    pub fn close() -> Self {
        Packet64 {
            id: 0,
            target: 0,
            body: Packer::Close(),
        }
    }
    pub fn str(id: u16, target: u16, s: String) -> Self {
        Packet64 {
            id,
            target,
            body: Packer::Str(s),
        }
    }
    // pub fn id(&self) -> u16 {
    //     self.id
    // }
    pub fn target(&self) -> &u16 {
        &self.target
    }
    pub fn body(&self) -> &Packer {
        &self.body
    }
    // pub fn body_mut(&mut self) -> &mut Packer {
    //     &mut self.body
    // }
    // pub fn to_bytes(&self) -> Result<Vec<u8>, Box<dyn Error>> {
    //     let mut buf = Vec::new();
    //     let mut ser = Serializer::new(&mut buf);
    //     self.serialize(&mut ser)?;
    //     Ok(buf)
    // }
    // pub fn from_bytes(data: &[u8]) -> Result<Self, Box<dyn Error>> {
    //     let mut de = Deserializer::new(data);
    //     let p = Self::deserialize(&mut de)?;
    //     Ok(p)
    // }
}

pub struct Packy {}
impl Decoder for Packy {
    type Item = Packet64;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            // Not enough data to read length marker.
            return Ok(None);
        }

        // Read length marker.
        let mut length_bytes = [0u8; 4];
        length_bytes.copy_from_slice(&src[..4]);
        let length = u32::from_le_bytes(length_bytes) as usize;

        // Check that the length is not too large to avoid a denial of
        // service attack where the server runs out of memory.
        if length > MAX_BYTES {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Frame of length {} is too large.", length),
            ));
        }

        if src.len() < 4 + length {
            // The full string has not yet arrived.
            //
            // We reserve more space in the buffer. This is not strictly
            // necessary, but is a good idea performance-wise.
            src.reserve(4 + length - src.len());

            // We inform the Framed that we need more bytes to form the next
            // frame.
            return Ok(None);
        }

        // Use advance to modify src such that it no longer contains
        // this frame.
        let data = &src[4..4 + length];
        let t = 0;

        let packet = match rmp_serde::from_read::<_, Packet64>(data) {
            Err(e) => return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Ok(f) => f,
        };
        // let packet = match t {
        //     0 => Packer::Str(String::from_utf8(data.to_vec()).unwrap()),
        //     // 1 => Packer::FVec(data.to_vec()),
        //     // 2 => Packer::UVec(data.to_vec()),
        //     _ => Packer::Str(String::from_utf8(data.to_vec()).unwrap()),
        // };
        // Packer::Str(String::from_utf8(data.to_vec()).unwrap())

        src.advance(4 + length);

        // Convert the data to a string, or fail if it is not valid utf-8.
        // let pp = match String::from_utf8(data) {
        //     Ok(string) => Ok(Some(string)),
        //     Err(utf8_error) => Err(std::io::Error::new(
        //         std::io::ErrorKind::InvalidData,
        //         utf8_error.utf8_error(),
        //     )),
        // };
        Ok(Some(packet))

        // let re = rmp_serde::from_read::<_, P>(data);
        // match re {
        //     Err(e) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
        //     Ok(f) => Ok(Some(f)),
        // }
    }
}

impl Encoder<Packet64> for Packy {
    type Error = std::io::Error;

    fn encode(&mut self, item: Packet64, dst: &mut BytesMut) -> Result<(), Self::Error> {
        // Don't send a string if it is longer than the other end will
        // accept.
        let re = rmp_serde::to_vec(&item);
        match re {
            Err(e) => Err(std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Ok(bytes) => {
                let len = bytes.len();
                if len > MAX_BYTES {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Frame of length {} is too large.", len),
                    ));
                }
                // item.serialize(serializer)

                // Convert the length into a byte array.
                // The cast to u32 cannot overflow due to the length check above.
                let len_slice = u32::to_le_bytes(len as u32);

                // Reserve space in the buffer.
                dst.reserve(4 + len);

                // Write the length and string to the buffer.
                dst.extend_from_slice(&len_slice);
                dst.extend_from_slice(&bytes);
                Ok(())
            }
        }
    }
}

// pub fn table_serial(table: Table) {
//     // table.raw_sequence_values()
//     let indexes: Vec<Value> = table
//         .clone()
//         .pairs::<Value, Value>()
//         .filter_map(|f| match f {
//             Ok((key, _)) => Some(key),
//             _ => None,
//         })
//         .collect();

//     let ind_len = indexes.len();

//     // match f.1 {
//     //     Value::Table(t) => table_serial(t),
//     //     Value::String(s) => println!("{} {}", f.0, s.to_str().unwrap()),
//     //     Value::Integer(i) => println!("{} {}", f.0, i),
//     //     _ => {}
//     // }

//     let seq = table.raw_sequence_values::<Value>();
//     let seq2 = seq
//         .filter_map(|f| match f {
//             Ok(r) => Some(r),
//             _ => None,
//         })
//         .collect_vec();
//     let seq_len = seq2.len();
//     if seq_len != ind_len {
//         panic!("length mismatch seq_len {} ind_len {}", seq_len, ind_len)
//     } else {
//         println!("length match!")
//     }
//     // length

//     // for i in 0..indexes.len() {
//     //     let key = indexes[i];
//     //     seq.
//     //     let value = table.get::<Value, Value>(&key).unwrap();
//     //     match value {
//     //         Value::Table(t) => table_serial(t),
//     //         Value::String(s) => println!("{} {}", key, s.to_str().unwrap()),
//     //         Value::Integer(i) => println!("{} {}", key, i),
//     //         _ => {}
//     //     }
//     // }

//     // for pair in table.raw_sequence_values::<Value, Value>() {
//     //     if let Ok((key, value)) = pair {
//     //         match value {
//     //             Value::Table(t) => table_serial(t),
//     //             Value::String(s) => s.to_str(),
//     //             Value::Integer(i) =>i.to_string(),
//     //             ,
//     //             _ => {}
//     //         }
//     //     }
//     // }
//     // for i in table.raw_sequence_values() {}
// }

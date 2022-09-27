use std::{
    collections::hash_map::Entry,
    sync::mpsc::{channel, sync_channel, Sender, SyncSender},
};

use glam::{ivec3, ivec4, IVec3, IVec4};
use wgpu::Device;

use crate::tile::{Chunk, ChunkModel, Layer, LayerModel};

/*
texture index of 0 is air and not rendered
texture index of 1 is "plain" and unmodifies texture
2..N is any texture index in the atlas
*/

pub enum TileCommand {
    // Make((String, String, String, String, String, String, String)),
    Set(Vec<(String, IVec4)>),
    Drop(IVec3),
    Check(Vec<String>),
    Is(IVec3),
    Reset(),
}

pub enum TileResponse {
    Success(bool),
    Chunks(Vec<Chunk>, bool),
}

pub struct World {
    layer: LayerModel,
    pub sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
}

impl World {
    pub fn new() -> World {
        // let path = [[0, 0, 0], [0, 0, 0], [0, 0, 0]];
        // let h = path.len();
        // let w = path[0].len();
        // let mut hash = FxHashMap::default();
        // hash.insert(4, 3);
        // hash.insert(2, 33);
        // hash.insert(13, 52);
        // hash.insert(1, 25);

        // for yi in 0..path.len() {
        //     let row = path[yi];
        //     for xi in 0..row.len() {
        //         let c = row[xi];
        //         let x = xi as i32 * 16;
        //         let y = yi as i32 * -16;
        //         if c == 1 {
        //             let u = if yi > 0 { path[yi - 1][xi] } else { 0 };
        //             let d = if yi < h - 1 { path[yi + 1][xi] } else { 0 };
        //             let l = if xi > 0 { path[yi][xi - 1] } else { 0 };
        //             let r = if xi < w - 1 { path[yi][xi + 1] } else { 0 };
        //             let bit = u | r << 1 | d << 2 | l << 3;
        //             let h = match hash.get(&bit) {
        //                 Some(n) => n.clone(),
        //                 _ => 34,
        //             };
        //             println!("x{} y{} b{}", xi, yi, bit);
        //             world.set_tile(&format!("map{}", h), x, y, 0);
        //         } else {
        //             world.set_tile(&format!("map{}", 44), x, y, 0); //36
        //         }
        //     }
        // }

        // for i in 0..16 {
        //     for j in 0..16 {
        //         world.set_tile(&format!("fatty"), (i - 8) * 16, (j - 8) * 16, -32 * 3)
        //     }
        // }
        // for i in 0..16 {
        //     for j in 0..16 {
        //         world.set_tile(&format!("grid"), 16 * 16, (i - 8) * 16, (j - 8) * 16)
        //     }
        // }

        // world.set_tile(&format!("grid"), 0, 0, 16 * 0);

        // world.get_chunk_mut(100, 100, 100).cook(device);

        World {
            layer: LayerModel::new(),
            sender: World::init(),
        }
    }

    pub fn start(&mut self) {
        self.sender = World::init();
    }

    pub fn init() -> Sender<(TileCommand, SyncSender<TileResponse>)> {
        // add block
        // check block
        // get chunks near point

        // if is dirty, rebuild
        let (sender, reciever) = channel::<(TileCommand, SyncSender<TileResponse>)>();
        // log("init lua core".to_string());

        // let world_thread =
        std::thread::spawn(move || {
            let mut layer = Layer::new();
            // let a_device = Arc::clone(&device);
            for m in reciever {
                match m {
                    // (TileCommand::Make((name, t, w, n, e, s, b)), response) => {
                    //     // log(format!("set tile {}", tiles[0].0));

                    //     // crate::model::edit_cube(name, vec![t, w, n, e, s, b], a_device);
                    //     res_handle(response.send(TileResponse::Success(true)))
                    // }
                    (TileCommand::Set(tiles), response) => {
                        // log(format!("set tile {}", tiles[0].0));
                        layer.set_tile(
                            &tiles[0].0.to_lowercase(),
                            tiles[0].1.x as u8,
                            tiles[0].1.y as i32,
                            tiles[0].1.z as i32,
                            tiles[0].1.w as i32,
                        );
                        res_handle(response.send(TileResponse::Success(true)))
                    }
                    (TileCommand::Check(_), response) => {
                        let chunks = layer.get_dirty();
                        // log(format!("chunks returned is {}", chunks.len()));
                        let dropped = layer.dropped;
                        if dropped {
                            println!("triggerd drop");
                            layer.dropped = false;
                        }
                        res_handle(response.send(TileResponse::Chunks(chunks, dropped)))
                    }
                    (TileCommand::Is(tile), response) => res_handle(
                        response.send(TileResponse::Success(layer.is_tile(tile.x, tile.y, tile.z))),
                    ),
                    (TileCommand::Drop(v), response) => {
                        layer.drop_chunk(v.x as i32, v.y as i32, v.z as i32);
                        res_handle(response.send(TileResponse::Success(true)))
                    }
                    (TileCommand::Reset(), response) => {
                        layer.destroy_it_all();
                        res_handle(response.send(TileResponse::Success(true)));
                    }
                }
            }
        });
        sender
    }

    pub fn get_chunk_models(
        &mut self,
        device: &Device,
    ) -> std::collections::hash_map::Values<String, ChunkModel> {
        let (tx, rx) = sync_channel::<TileResponse>(0);

        match self.sender.send((TileCommand::Check(vec![]), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Chunks(chunks, dropped)) => {
                    if dropped && chunks.is_empty() {
                        // for (s, c) in self.layer.chunks.drain() {
                        //     // c.build_chunk(chunk);
                        //     // c.cook(device);
                        // }
                        self.layer.chunks.clear()
                    } else {
                        for chunk in chunks {
                            // let key = chunk.key.clone();
                            match self.layer.chunks.entry(chunk.key.clone()) {
                                Entry::Occupied(o) => {
                                    let c = o.into_mut();
                                    // println!("rebuild model chunk {}", chunk.key);
                                    c.build_chunk(chunk);
                                    c.cook(device);
                                }
                                Entry::Vacant(v) => {
                                    let ix = chunk.pos.x.div_euclid(16) * 16;
                                    let iy = chunk.pos.y.div_euclid(16) * 16;
                                    let iz = chunk.pos.z.div_euclid(16) * 16;
                                    // println!(
                                    //     "populate new model chunk {} {} {} {}",
                                    //     chunk.key, ix, iy, iz
                                    // );
                                    let mut model =
                                        ChunkModel::new(device, chunk.key.clone(), ix, iy, iz);
                                    model.build_chunk(chunk);
                                    model.cook(device);
                                    v.insert(model);
                                }
                            }
                        }
                    }
                }
                _ => {}
            },
            _ => {}
        }

        //MutexGuard::unlock_fair(guard);

        self.layer.chunks.values()
    }

    pub fn set_tile(
        sender: &Sender<(TileCommand, SyncSender<TileResponse>)>,
        t: String,
        x: i32,
        y: i32,
        z: i32,
        r: u8,
    ) {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        match sender.send((TileCommand::Set(vec![(t, ivec4(r as i32, x, y, z))]), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Success(true)) => {}
                _ => {}
            },
            _ => {}
        }
    }
    pub fn drop_chunk(
        sender: &Sender<(TileCommand, SyncSender<TileResponse>)>,
        x: i32,
        y: i32,
        z: i32,
    ) {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        match sender.send((TileCommand::Drop(ivec3(x, y, z)), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Success(true)) => {}
                _ => {}
            },
            _ => {}
        }
    }

    pub fn is_tile(
        sender: &Sender<(TileCommand, SyncSender<TileResponse>)>,
        x: i32,
        y: i32,
        z: i32,
    ) -> bool {
        let (tx, rx) = sync_channel::<TileResponse>(0);

        match sender.send((TileCommand::Is(ivec3(x, y, z)), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Success(b)) => b,
                _ => false,
            },
            _ => false,
        }
    }

    // pub fn make(
    //     sender: &Sender<(TileCommand, SyncSender<TileResponse>)>,
    //     name: String,
    //     t: String,
    //     b: String,
    //     e: String,
    //     w: String,
    //     s: String,
    //     n: String,
    // ) {
    // }

    pub fn clear_tiles(sender: &Sender<(TileCommand, SyncSender<TileResponse>)>) {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        match sender.send((TileCommand::Reset(), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Success(true)) => {
                    crate::lg!("cleared tiles")
                }
                _ => {}
            },
            _ => {}
        }
    }

    pub fn destroy_it_all(&mut self) {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        match self.sender.send((TileCommand::Reset(), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Success(true)) => {}
                _ => {}
            },
            _ => {}
        }

        self.layer.destroy_it_all();
    }

    // pub fn build_chunk_from_pos(&mut self, ix: i32, iy: i32, iz: i32) {
    //     let c = self.get_chunk_mut(ix, iy, iz);
    //     self.build_chunk2(c);
    // }

    // pub fn build_dirty_chunks(&mut self, device: &Device) {
    //     let mut d = vec![];
    //     for c in self.layer.get_all_chunks() {
    //         if c.dirty {
    //             d.push(c.key.clone());
    //         }
    //     }
    //     println!("we found {} dirty chunk(s)", d.len());
    //     for k in d {
    //         match self.layer.get_chunk_mut(k) {
    //             Some(c) => {
    //                 c.build_chunk();
    //                 c.cook(device);
    //             }
    //             _ => {}
    //         }
    //     }
    // }

    // pub fn add_tile_model(&mut self, texture: &String, model: &String, ix: i32, iy: i32, iz: i32) {
    //     let c = self.get_chunk_mut(ix, iy, iz);
    //     _add_tile_model(c, texture, model, ix, iy, iz);
    // }
}

fn res_handle(res: Result<(), std::sync::mpsc::SendError<TileResponse>>) {
    match res {
        Ok(_) => {}
        Err(er) => {
            log(format!("Failed to respond::{}", er.to_string()));
        }
    }
}

pub fn log(s: String) {
    println!("World::{}", s);
    crate::log::log(format!("World::{}", s));
}

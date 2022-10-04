use std::{
    collections::{hash_map::Entry, HashMap},
    rc::Rc,
    sync::mpsc::{channel, sync_channel, Sender, SyncSender},
};

use glam::{ivec3, ivec4, vec4, IVec3, IVec4, Vec4};
use rustc_hash::FxHashMap;
use wgpu::Device;

use crate::{
    model::{Model, ModelManager},
    tile::{Chunk, ChunkModel, Layer, LayerModel},
    Core,
};

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
    /** Simply get whether a tile is present at position or not */
    Is(IVec3),
    /** Apply a texture string and it's vector uv map to our hash and create new index within the world if it doesn't exist*/
    MapTex(String, u32),
    /** Apply a model string and it's arc<model> to our hash and create new index within the world if it doesn't exist*/
    MapModel(String),
    Reset(),
}

pub enum TileResponse {
    Success(bool),
    Mapped(u32),
    Chunks(Vec<Chunk>, bool),
}

pub struct WorldInstance {
    /** a really basic UUID, just incrementing from 0..n is fine internally, can we even go higher then 4294967295 (2^32 -1 is u32 max)?*/
    pub COUNTER: u32,
    /** map our texture strings to their integer value for fast lookup and 3d grid insertion*/
    pub INT_TEX_DICTIONARY: HashMap<String, u32>,
    // pub INT_TEX_MAP: FxHashMap<u32, Vec4>,
    pub INT_MODEL_DICTIONARY: HashMap<String, u32>,
    // pub INT_MODEL_MAP: FxHashMap<u32, Arc<OnceCell<Model>>>,
}

impl WorldInstance {
    pub fn new() -> WorldInstance {
        WorldInstance {
            COUNTER: 2,
            INT_TEX_DICTIONARY: HashMap::new(),
            // INT_TEX_MAP: FxHashMap::default(),
            INT_MODEL_DICTIONARY: HashMap::new(),
            // INT_MODEL_MAP: FxHashMap::default(),
        }
    }

    /** return texture numerical index from a given texture name */
    pub fn get_tex_index(&self, str: &String) -> u32 {
        match self.INT_TEX_DICTIONARY.get(str) {
            Some(n) => n.clone(),
            _ => 1,
        }
    }
    /** return texture uv coordinates and numerical index from a given texture name */
    // pub fn get_tex_and_index(str: &String) {}

    /** Create a simple numerical key for our texture and uv and map it, returning that numerical key*/
    fn index_texture(&mut self, key: String) -> u32 {
        let ind = self.COUNTER;
        self.INT_TEX_DICTIONARY.insert(key, ind);
        self.COUNTER += 1;
        ind
    }

    /** We already had a numerical key created in a previous index_texture call and want to add another String->u32 translation*/
    fn index_texture_direct(&mut self, key: String, index: u32) {
        self.INT_TEX_DICTIONARY.insert(key, index);
    }

    /** return model numerical index from a given model name */
    pub fn get_model_index(&self, str: &String) -> Option<u32> {
        match self.INT_MODEL_DICTIONARY.get(str) {
            Some(u) => {
                // log(format!("🟢ye we got {} from {}", u, str));
                Some(u.clone())
            }
            None => None,
        }
    }

    /** Create a simple numerical key for our model and map it, returning that numerical key*/
    fn index_model(&mut self, key: String) -> u32 {
        let ind = self.COUNTER;
        self.INT_MODEL_DICTIONARY.insert(key, ind);
        self.COUNTER += 1;
        ind
    }
}
pub struct World {
    layer: LayerModel,
    pub sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
    pub local_tex_map: FxHashMap<u32, Vec4>,
    pub local_model_map: FxHashMap<u32, Rc<Model>>,
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
            local_tex_map: FxHashMap::default(),
            local_model_map: FxHashMap::default(),
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
        let mut model_map_dirty = false;
        let mut tex_map_dirty = false;

        std::thread::spawn(move || {
            let mut layer = Layer::new();
            let mut instance = WorldInstance::new();

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
                            &instance,
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

                        // if dropped && chunks.is_empty() {
                        // } else {
                        //     for chunk in chunks {
                        //         // let key = chunk.key.clone();
                        //         match self.layer.chunks.entry(chunk.key.clone()) {
                        //             Entry::Occupied(o) => {
                        //                 let c = o.into_mut();
                        //                 // println!("rebuild model chunk {}", chunk.key);
                        //                 c.build_chunk(instance, chunk);
                        //                 c.cook(device);
                        //             }
                        //             Entry::Vacant(v) => {
                        //                 let ix = chunk.pos.x.div_euclid(16) * 16;
                        //                 let iy = chunk.pos.y.div_euclid(16) * 16;
                        //                 let iz = chunk.pos.z.div_euclid(16) * 16;
                        //                 // println!(
                        //                 //     "populate new model chunk {} {} {} {}",
                        //                 //     chunk.key, ix, iy, iz
                        //                 // );
                        //                 let mut model =
                        //                     ChunkModel::new(device, chunk.key.clone(), ix, iy, iz);
                        //                 model.build_chunk(instance, chunk);
                        //                 model.cook(device);
                        //                 v.insert(model);
                        //             }
                        //         }
                        //     }
                        // }

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
                    (TileCommand::MapTex(name, direct), response) => {
                        let i = if direct > 0 {
                            instance.index_texture(name)
                        } else {
                            instance.index_texture_direct(name, direct);
                            direct
                        };

                        res_handle(response.send(TileResponse::Mapped(i)));
                    }
                    (TileCommand::MapModel(name), response) => {
                        let i = instance.index_model(name);
                        res_handle(response.send(TileResponse::Mapped(i)));
                    }
                }
            }
        });
        sender
    }

    pub fn get_chunk_models(
        &mut self,
        model_manager: &ModelManager,
        device: &Device,
    ) -> std::collections::hash_map::Values<String, ChunkModel> {
        let (tx, rx) = sync_channel::<TileResponse>(0);

        match self.sender.send((TileCommand::Check(vec![]), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Chunks(chunks, dropped)) => {
                    if dropped && chunks.is_empty() {
                        self.layer.chunks.clear()
                    } else {
                        for chunk in chunks {
                            // let key = chunk.key.clone();
                            match self.layer.chunks.entry(chunk.key.clone()) {
                                Entry::Occupied(o) => {
                                    let c = o.into_mut();
                                    // println!("rebuild model chunk {}", chunk.key);
                                    c.build_chunk(
                                        &self.local_tex_map,
                                        &self.local_model_map,
                                        model_manager,
                                        chunk,
                                    );
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
                                    model.build_chunk(
                                        &self.local_tex_map,
                                        &self.local_model_map,
                                        model_manager,
                                        chunk,
                                    );
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

    /** index the texture uv by name  within the world, and the world instance generates an index by name to store on it's own thread*/
    pub fn index_texture(&mut self, name: String, uv: Vec4) -> u32 {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        match self.sender.send((TileCommand::MapTex(name, 0), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Mapped(i)) => {
                    self.local_tex_map.insert(i, uv);
                    i
                }
                _ => 0,
            },
            _ => 0,
        }
    }
    /** We already have an index for a texture we're just providing a string alias */
    pub fn index_texture_alias(&self, name: String, direct: u32) {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        match self.sender.send((TileCommand::MapTex(name, direct), tx)) {
            Ok(_) => match rx.recv() {
                _ => {}
            },
            _ => {}
        }
    }

    /** index the model by name within the world, and the world instance generates an index by name to store on it's own thread*/
    pub fn index_model(&mut self, name: String, model: Rc<Model>) {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        match self.sender.send((TileCommand::MapModel(name), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Mapped(i)) => {
                    // self.local_tex_map.insert(k, v)
                    self.local_model_map.insert(i, model);
                }
                _ => {}
            },
            _ => {}
        }
    }

    /** return texture uv coordinates from a given texture numerical index */
    pub fn get_tex_from_index(&self, ind: u32) -> Vec4 {
        match self.local_tex_map.get(&ind) {
            Some(uv) => uv.clone(),
            _ => vec4(1., 1., 0., 0.),
        }
    }

    pub fn get_model_from_index(&self, index: u32) -> Option<Rc<Model>> {
        match self.local_model_map.get(&index) {
            Some(model) => Some(Rc::clone(model)),

            None => None,
        }
    }
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

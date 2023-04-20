use std::{
    collections::{hash_map::Entry, HashMap},
    rc::Rc,
    sync::mpsc::{channel, sync_channel, Sender, SyncSender},
};

use glam::{ivec3, ivec4, vec4, IVec3, IVec4, Vec4};
use rustc_hash::FxHashMap;
#[cfg(feature = "headed")]
use wgpu::Device;

use crate::{
    command::MainCommmand,
    log::LogType,
    model::{Model, ModelManager},
    tile::{Chunk, ChunkModel, Layer, LayerModel},
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
    /** Retrieve changes before rendering */
    Check(),
    /** Get string model asset at position if any */
    Get(IVec3),
    /** Simply get whether a tile is present at position or not, slightly cheaper then Get */
    Is(IVec3),
    /** Find first occurence in a direction until limit is reached*/
    First(Option<String>, IVec3, IVec3, u32),
    /* Compute a path from point A to B with a integer gravity (0,0,0 for none) a step size, 0 for flat, and a limit to search before giving up */
    Path(IVec3, IVec3, IVec3, u32, u32),
    /** Apply a mobility score for a block type */
    MapScore(String, u32),
    //WFC()
    /** Apply a texture string and it's vector uv map to our hash and create new index within the world if it doesn't exist*/
    MapTex(String, u32),
    /** Apply a model string and it's arc<model> to our hash and create new index within the world if it doesn't exist*/
    MapModel(String),
    Clear(),
    Destroy(),
    Stats(),
}

pub enum TileResponse {
    Success(bool),
    Mapped(u32),
    Found(Option<(String, u8)>),
    Chunks(Vec<Chunk>, bool),
    Location(Option<[i32; 3]>),
}

pub struct WorldInstance {
    pub bundle_id: u8,
    /** a really basic UUID, just incrementing from 0..n is fine internally, can we even go higher then 4294967295 (2^32 -1 is u32 max)?*/
    pub COUNTER: u32,
    /** map our texture strings to their integer value for fast lookup and 3d grid insertion*/
    pub INT_TEX_DICTIONARY: HashMap<String, u32>,
    pub INT_TEX_REVERSE_DICTIONARY: HashMap<u32, String>,
    // pub INT_TEX_MAP: FxHashMap<u32, Vec4>,
    pub INT_MODEL_DICTIONARY: HashMap<String, u32>,
    pub INT_MODEL_REVERSE_DICTIONARY: HashMap<u32, String>,
    // pub INT_MODEL_MAP: FxHashMap<u32, Arc<OnceCell<Model>>>,
}

impl WorldInstance {
    pub fn new(bundle_id: u8) -> WorldInstance {
        WorldInstance {
            bundle_id,
            COUNTER: 2,
            INT_TEX_DICTIONARY: HashMap::new(),
            INT_TEX_REVERSE_DICTIONARY: HashMap::new(),
            // INT_TEX_MAP: FxHashMap::default(),
            INT_MODEL_DICTIONARY: HashMap::new(),
            INT_MODEL_REVERSE_DICTIONARY: HashMap::new(),
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
        self.INT_TEX_DICTIONARY.insert(key.clone(), ind);
        self.INT_TEX_REVERSE_DICTIONARY.insert(ind, key);
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
    pub fn get_model_by_index(&self, index: u32) -> Option<String> {
        match self.INT_MODEL_REVERSE_DICTIONARY.get(&index) {
            Some(u) => Some(u.clone()),
            None => None,
        }
    }
    pub fn get_tex_by_index(&self, index: u32) -> Option<String> {
        match self.INT_TEX_REVERSE_DICTIONARY.get(&index) {
            Some(u) => Some(u.clone()),
            None => None,
        }
    }

    /** Create a simple numerical key for our model and map it, returning that numerical key*/
    fn index_model(&mut self, key: String) -> u32 {
        let ind = self.COUNTER;
        self.INT_MODEL_DICTIONARY.insert(key.clone(), ind);
        self.INT_MODEL_REVERSE_DICTIONARY.insert(ind, key);
        self.COUNTER += 1;
        ind
    }
    pub fn stats(&self) {
        println!("++World_Instance_{}---------------------", self.bundle_id);
        self.INT_TEX_DICTIONARY.iter().for_each(|(k, v)| {
            println!("---tex2int {}->{}", k, v);
        });
        self.INT_MODEL_DICTIONARY.iter().for_each(|(k, v)| {
            println!("---model2int {}->{}", k, v);
        });
    }
}
pub struct Mapper {
    tex_map: FxHashMap<u32, Vec4>,
    model_map: FxHashMap<u32, Rc<Model>>,
}
impl Mapper {
    pub fn new() -> Mapper {
        Mapper {
            tex_map: FxHashMap::default(),
            model_map: FxHashMap::default(),
        }
    }
    // pub fn get_tex(&self, index: u32) -> Vec4{
    //     match self.tex_map.get(&index){
    //         Some(v) => v.clone(),
    //         None => vec4(0.0, 0.0, 1.0, 1.0),
    //     }
    // }
    // pub fn get_model(&self, index: u32) -> Rc<Model>{
    //     match self.model_map.get(&index){
    //         Some(v) => v.clone(),
    //         None => Rc::new(Model::new()),
    //     }
    // }
    // pub fn set_tex(&mut self, index: u32, uv: Vec4){
    //     self.tex_map.insert(index, uv);
    // }
    // pub fn set_model(&mut self, index: u32, model: Rc<Model>){
    //     self.model_map.insert(index, model);
    // }
}
pub struct World {
    layers: FxHashMap<u8, LayerModel>,
    pub senders: FxHashMap<u8, Sender<(TileCommand, SyncSender<TileResponse>)>>,
    pub local_mappers: FxHashMap<u8, Mapper>,
    loggy: Sender<(LogType, String)>,
}

impl World {
    pub fn new(loggy: Sender<(LogType, String)>) -> World {
        // let senders = FxHashMap::default();
        // senders.insert(k, v)
        World {
            layers: FxHashMap::default(),
            senders: FxHashMap::default(),
            local_mappers: FxHashMap::default(),
            loggy,
        }
    }

    pub fn make(
        &mut self,
        bundle_id: u8,
        pitcher: Sender<(u8, MainCommmand)>,
    ) -> Sender<(TileCommand, SyncSender<TileResponse>)> {
        println!("🟢World::make");
        let sender = World::init(bundle_id, pitcher, self.loggy.clone());
        self.senders.insert(bundle_id, sender.clone());
        self.layers.insert(bundle_id, LayerModel::new());
        self.local_mappers.insert(bundle_id, Mapper::new());
        sender
    }

    pub fn init(
        bundle_id: u8,
        pitcher: Sender<(u8, MainCommmand)>,
        loggy: Sender<(LogType, String)>,
    ) -> Sender<(TileCommand, SyncSender<TileResponse>)> {
        // add block
        // check block
        // get chunks near point
        // if is dirty, rebuild

        let (sender, reciever) = channel::<(TileCommand, SyncSender<TileResponse>)>();

        std::thread::spawn(move || {
            let mut layer = Layer::new();
            let mut instance = WorldInstance::new(bundle_id);

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
                        res_handle(response.send(TileResponse::Success(true)), &loggy)
                    }
                    (TileCommand::Check(), response) => {
                        let chunks = layer.get_dirty();
                        let dropped = layer.dropped;
                        if dropped {
                            println!("triggerd drop");
                            layer.dropped = false;
                        }
                        if !chunks.is_empty() || dropped {
                            pitcher.send((bundle_id, MainCommmand::WorldSync(chunks, dropped)));
                        }
                    }
                    (TileCommand::Is(tile), response) => res_handle(
                        response.send(TileResponse::Success(layer.is_tile(tile.x, tile.y, tile.z))),
                        &loggy,
                    ),
                    (TileCommand::Drop(v), response) => {
                        layer.drop_chunk(v.x as i32, v.y as i32, v.z as i32);
                        res_handle(response.send(TileResponse::Success(true)), &loggy)
                    }
                    (TileCommand::Destroy(), response) => {
                        layer.destroy_it_all();
                        res_handle(response.send(TileResponse::Success(true)), &loggy);
                        break;
                    }
                    (TileCommand::Clear(), response) => {
                        layer.destroy_it_all();
                        res_handle(response.send(TileResponse::Success(true)), &loggy);
                    }
                    (TileCommand::MapTex(name, direct), response) => {
                        let i = if direct <= 0 {
                            instance.index_texture(name)
                        } else {
                            instance.index_texture_direct(name, direct);
                            direct
                        };

                        res_handle(response.send(TileResponse::Mapped(i)), &loggy);
                    }
                    (TileCommand::MapModel(name), response) => {
                        let i = instance.index_model(name);
                        res_handle(response.send(TileResponse::Mapped(i)), &loggy);
                    }
                    (TileCommand::Stats(), response) => {
                        instance.stats();
                        layer.stats();
                        res_handle(response.send(TileResponse::Success(true)), &loggy);
                    }
                    (TileCommand::Get(tile), response) => {
                        let res =
                            layer.get_tile(&instance, tile.x as i32, tile.y as i32, tile.z as i32);
                        res_handle(response.send(TileResponse::Found(res)), &loggy)
                    }
                    (TileCommand::First(tile, pos, dir, limit), response) => {
                        let res = layer.first_tile(
                            &instance, tile, pos.x, pos.y, pos.z, dir.x, dir.y, dir.z, limit,
                        );
                        res_handle(response.send(TileResponse::Location(res)), &loggy)
                    }
                    _ => {
                        todo!("world command")
                    }
                }
            }
        });
        sender
    }

    pub fn process_sync(
        &mut self,
        #[cfg(feature = "headed")] device: &Device,
        bundle_id: u8,
        chunks: Vec<Chunk>,
        dropped: bool,
        model_manager: &ModelManager,
    ) {
        if let Some(layer) = self.layers.get_mut(&bundle_id) {
            if dropped && chunks.is_empty() {
                layer.chunks.clear();
            } else {
                if let Some(mapper) = self.local_mappers.get(&bundle_id) {
                    for chunk in chunks {
                        // let key = chunk.key.clone();
                        match layer.chunks.entry(chunk.key.clone()) {
                            Entry::Occupied(o) => {
                                let c = o.into_mut();
                                // println!("rebuild model chunk {}", chunk.key);
                                c.build_chunk(
                                    &mapper.tex_map,
                                    &mapper.model_map,
                                    model_manager,
                                    chunk,
                                );
                                #[cfg(feature = "headed")]
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
                                let mut model = ChunkModel::new(
                                    #[cfg(feature = "headed")]
                                    device,
                                    chunk.key.clone(),
                                    ix,
                                    iy,
                                    iz,
                                );
                                model.build_chunk(
                                    &mapper.tex_map,
                                    &mapper.model_map,
                                    model_manager,
                                    chunk,
                                );
                                #[cfg(feature = "headed")]
                                model.cook(device);
                                v.insert(model);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn get_chunk_models(&mut self) -> Vec<&ChunkModel> {
        let (tx, rx) = sync_channel::<TileResponse>(0);

        // ping all instances asyncronously
        self.senders.values().for_each(|sender| {
            match sender.send((TileCommand::Check(), tx.clone())) {
                Ok(_) => {}
                Err(e) => {
                    // println!("error sending check command: {}", e);
                }
            }
        });
        // println!("layers #{}", self.layers.len());
        // return the current layer layout
        self.layers
            .values()
            .flat_map(|l| l.chunks.values())
            .collect::<Vec<&ChunkModel>>()
        // self.layer.chunks.values()
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
                Ok(TileResponse::Success(b)) => {
                    // println!("is tile {} {} {} {}", x, y, z, b);
                    b
                }
                _ => false,
            },
            _ => false,
        }
    }
    pub fn get_tile(
        sender: &Sender<(TileCommand, SyncSender<TileResponse>)>,
        x: i32,
        y: i32,
        z: i32,
    ) -> Option<(String, u8)> {
        let (tx, rx) = sync_channel::<TileResponse>(0);

        match sender.send((TileCommand::Get(ivec3(x, y, z)), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Found(s)) => {
                    // println!("is tile {} {} {} {}", x, y, z, b);
                    s
                }
                _ => None,
            },
            _ => None,
        }
    }
    pub fn first_tile(
        sender: &Sender<(TileCommand, SyncSender<TileResponse>)>,
        tile: Option<String>,
        x: i32,
        y: i32,
        z: i32,
        dx: i32,
        dy: i32,
        dz: i32,
        limit: u32,
    ) -> Option<[i32; 3]> {
        let (tx, rx) = sync_channel::<TileResponse>(0);

        match sender.send((
            TileCommand::First(tile, ivec3(x, y, z), ivec3(dx, dy, dz), limit),
            tx,
        )) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Location(l)) => {
                    // println!("is tile {} {} {} {}", x, y, z, b);
                    l
                }
                _ => None,
            },
            _ => None,
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

    /** Clear a world instance of it's tiles and models but keep active */
    pub fn clear_tiles(sender: &Sender<(TileCommand, SyncSender<TileResponse>)>) {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        match sender.send((TileCommand::Clear(), tx)) {
            Ok(_) => match rx.recv() {
                Ok(TileResponse::Success(true)) => {
                    // crate::lg!("cleared tiles")
                }
                _ => {}
            },
            _ => {}
        }
    }

    // pub fn clear(&mut self, bundle_id: u8) {
    //     match self.senders.remove(&bundle_id) {
    //         Some(sender) => {
    //             let (tx, rx) = sync_channel::<TileResponse>(0);
    //             match sender.send((TileCommand::Clear(), tx)) {
    //                 Ok(_) => match rx.recv() {
    //                     Ok(TileResponse::Success(true)) => {
    //                         crate::lg!("destroyed world instance {}", bundle_id)
    //                     }
    //                     _ => {}
    //                 },
    //                 _ => {}
    //             }
    //         }
    //         _ => {}
    //     }
    //     if let Some(layer) = self.layers.get(&bundle_id) {
    //         layer.chunks.clear();
    //     }
    // }

    /** Destroy a world instance and clear models as well as end the instance thread*/
    pub fn destroy(&mut self, bundle_id: u8) {
        match self.senders.remove(&bundle_id) {
            Some(sender) => {
                let (tx, rx) = sync_channel::<TileResponse>(0);
                match sender.send((TileCommand::Destroy(), tx)) {
                    Ok(_) => match rx.recv() {
                        Ok(TileResponse::Success(true)) => {
                            self.loggy.send((
                                LogType::World,
                                format!("destroyed world instance {}", bundle_id),
                            ));
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
            _ => {}
        }
        self.local_mappers.remove(&bundle_id);
        self.layers.remove(&bundle_id);
    }

    /** Destroy all world instances and clear all models. End all threads*/
    pub fn destroy_it_all(&mut self) {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        let mut remaining = self.senders.len();
        for (b, sender) in self.senders.drain() {
            match sender.send((TileCommand::Destroy(), tx.clone())) {
                Ok(_) => {}
                Err(e) => {
                    remaining -= 1;
                    // println!("error sending destroy command: {}", e);
                }
            }
        }
        // wait for all threads to end before completing function syncronously
        for r in rx {
            remaining -= 1;
            if remaining == 0 {
                break;
            }
        }
        self.local_mappers.clear();
        self.layers.clear()
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
    pub fn index_texture(&mut self, bundle_id: u8, name: String, uv: Vec4) -> u32 {
        match self.senders.get(&bundle_id) {
            Some(sender) => {
                let (tx, rx) = sync_channel::<TileResponse>(0);
                match sender.send((TileCommand::MapTex(name.to_lowercase(), 0), tx)) {
                    Ok(_) => match rx.recv() {
                        Ok(TileResponse::Mapped(i)) => {
                            // println!("mapped tex {} to {}", name, i);
                            match self.local_mappers.get_mut(&bundle_id) {
                                Some(m) => {
                                    m.tex_map.insert(i, uv);
                                }
                                _ => {
                                    self.loggy.send((
                                        LogType::WorldError,
                                        format!("no world tex mapper for bundle {}", bundle_id),
                                    ));
                                } // self.local_tex_map.insert(i, uv);
                            }
                            i
                        }
                        _ => 0,
                    },
                    _ => 0,
                }
            }
            _ => {
                self.loggy.send((
                    LogType::WorldError,
                    format!("err::no world tex mapper for bundle {}", bundle_id),
                ));
                0
            }
        }
    }

    /** We already have an index for a texture we're just providing a string alias */
    pub fn index_texture_alias(&mut self, bundle_id: u8, name: String, direct: u32) {
        if let Some(sender) = self.senders.get(&bundle_id) {
            let (tx, rx) = sync_channel::<TileResponse>(0);
            match sender.send((TileCommand::MapTex(name.to_lowercase(), direct), tx)) {
                Ok(_) => {
                    if let Ok(TileResponse::Mapped(i)) = rx.recv() {
                        // println!("mapped tex {} to {}", name, i);
                        match self.local_mappers.get_mut(&bundle_id) {
                            Some(m) => {
                                m.tex_map.insert(
                                    i,
                                    m.tex_map
                                        .get(&direct)
                                        .unwrap_or(&vec4(0., 0., 1., 1.))
                                        .clone(),
                                );
                            }
                            _ => {
                                self.loggy.send((
                                    LogType::WorldError,
                                    format!("no world tex mapper for bundle {}", bundle_id),
                                ));
                            }
                        }

                        // self.local_tex_map
                        //     .insert(i, self.local_tex_map.get(&direct).unwrap().clone());
                    }
                }
                _ => {
                    self.loggy.send((
                        LogType::WorldError,
                        format!("err::no world tex mapper for bundle {}", bundle_id),
                    ));
                }
            }
        }
    }

    /** index the model by name within the world, and the world instance generates an index by name to store on it's own thread*/
    pub fn index_model(&mut self, bundle_id: u8, name: &str, model: Rc<Model>) {
        let (tx, rx) = sync_channel::<TileResponse>(0);
        if let Some(sender) = self.senders.get(&bundle_id) {
            if let Ok(_) = sender.send((TileCommand::MapModel(name.to_lowercase()), tx)) {
                if let Ok(TileResponse::Mapped(i)) = rx.recv() {
                    // println!("mapped model {} to {}", name, i);
                    // self.local_model_map.insert(i, model);
                    match self.local_mappers.get_mut(&bundle_id) {
                        Some(m) => {
                            m.model_map.insert(i, model);
                        }
                        _ => {
                            self.loggy.send((
                                LogType::WorldError,
                                format!("no world model mapper for bundle {}", bundle_id),
                            ));
                        }
                    };
                }
            }
        }
    }
    // TODO should we get mapper every time?
    /** return texture uv coordinates from a given texture numerical index */
    // pub fn get_tex_from_index(&self, bundle_id: u8, ind: u32) -> Vec4 {
    //     match self.local_mappers.get(&bundle_id) {
    //         Some(m) => match m.tex_map.get(&ind) {
    //             Some(uv) => uv.clone(),
    //             _ => vec4(1., 1., 0., 0.),
    //         },
    //         _ => {
    //             lg!("err::no world tex mapper for bundle {}", bundle_id);
    //             vec4(1., 1., 0., 0.)
    //         }
    //     }
    // }

    // TODO should we get mapper every time?
    // pub fn get_model_from_index(&self, bundle_id: u8, index: u32) -> Option<Rc<Model>> {
    //     match self.local_mappers.get(&bundle_id) {
    //         Some(m) => match m.model_map.get(&index) {
    //             Some(model) => Some(Rc::clone(model)),
    //             None => None,
    //         },
    //         None => None,
    //     }
    // }
    pub fn stats(&self) {
        println!("+World====================");
        self.local_mappers.iter().for_each(|(k, v)| {
            println!("--mapper id {}", k);
            println!("---#int2models {}", v.model_map.len());
            v.model_map.iter().for_each(|(k, v)| {
                println!("---int2model {}->{}", k, v.name);
            });
            println!("---#int2tex {}", v.tex_map.len());
            v.tex_map.iter().for_each(|(k, v)| {
                println!("---int2tex {}->{}", k, v);
            });
        });

        println!("--#render_layers {}", self.layers.len());
        self.layers.iter().for_each(|(k, v)| {
            println!("---render_layer [{}] -> #{}", k, v.chunks.len());
            v.chunks.iter().enumerate().for_each(|(i, (ck, cv))| {
                println!(
                    "----render_chunk {} [{}] -> p:{}, v#:{}, i#:{}",
                    i,
                    ck,
                    cv.pos,
                    cv.vert_data.len(),
                    cv.ind_data.len()
                );
            });
        });
        self.senders.iter().for_each(|(k, v)| {
            let (tx, rx) = sync_channel::<TileResponse>(0);
            if let Ok(r) = v.send((TileCommand::Stats(), tx)) {
                // we don't have to wait this is just to preserve output order
                rx.recv();
            }
            // println!("--sender id{}-> #{}", k, v.len());
        });
    }
}

fn res_handle(
    res: Result<(), std::sync::mpsc::SendError<TileResponse>>,
    loggy: &Sender<(LogType, String)>,
) {
    match res {
        Ok(_) => {}
        Err(er) => {
            loggy.send((
                LogType::WorldError,
                format!("Failed to respond::{}", er.to_string()),
            ));
        }
    }
}

// pub fn log(s: String) {
//     println!("World::{}", s);
//     crate::log::log(format!("World::{}", s));
// }

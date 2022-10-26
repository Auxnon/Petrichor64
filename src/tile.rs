use rustc_hash::FxHashMap;
use std::{collections::hash_map::Entry, ops::Mul, rc::Rc};

use glam::{ivec3, IVec3, Mat4, Vec4};
use wgpu::{util::DeviceExt, Buffer, Device};

use crate::{
    ent::EntityUniforms,
    model::{Model, ModelManager, Vertex},
    world::WorldInstance,
};

const CHUNK_SIZE: i32 = 32;
pub struct Layer {
    /** if all world tiles were cleared */
    pub dropped: bool,
    chunks: FxHashMap<String, Chunk>,
}

impl Layer {
    pub fn new() -> Layer {
        Layer {
            dropped: false,
            chunks: FxHashMap::default(), // ![Chunk::new()],
        }
    }

    pub fn get_dirty(&mut self) -> Vec<Chunk> {
        let mut v: Vec<Chunk> = vec![];
        for c in self.get_all_chunks_mut() {
            if c.dirty {
                v.push(c.clone());
                c.dirty = false;
            }
        }
        v
    }

    /** get a mutatable chunk, and create a new one if does not exist */
    pub fn get_chunk_mut(&mut self, x: i32, y: i32, z: i32) -> &mut Chunk {
        self.get_chunk_mut_from_pos(x, y, z)
    }

    /** read-only chunk, may not exist, will not create a new one if so */
    pub fn get_chunk(&self, x: i32, y: i32, z: i32) -> Option<&Chunk> {
        self.get_chunk_from_pos(x, y, z)
    }

    /** Set 1 tile in a chunk, if chunk doesn't exist it will create it and set the new tile */
    pub fn set_tile(
        &mut self,
        instance: &WorldInstance,
        tile: &String,
        meta: u8,
        ix: i32,
        iy: i32,
        iz: i32,
    ) {
        let cell_type = match instance.get_model_index(&tile) {
            Some(model_index) => model_index,
            _ => instance.get_tex_index(&tile),
        };

        let mut c = self.get_chunk_mut(ix, iy, iz);
        let index = ((((ix.rem_euclid(CHUNK_SIZE) * CHUNK_SIZE) + iy.rem_euclid(CHUNK_SIZE))
            * CHUNK_SIZE)
            + iz.rem_euclid(CHUNK_SIZE)) as usize;
        // let index = ((((ix / 16).rem_euclid(16)) * 16 + ((iy / 16).rem_euclid(16))) * 16
        //     + ((iz / 16).rem_euclid(16))) as usize;
        c.cells[index] = (cell_type, meta);
        // println!(
        //     "cell {:?} {} {} {} --- {} {} {}",
        //     index,
        //     ix.rem_euclid(16) * 256,
        //     iy.rem_euclid(16) * 16,
        //     iz.rem_euclid(16),
        //     ix,
        //     iy,
        //     iz
        // );
        c.dirty = true;
    }

    /** Set multiple tiles from a large array */
    pub fn set_tile_from_buffer(&mut self, instance: &WorldInstance, buffer: &Vec<(String, Vec4)>) {
        for (s, t) in buffer {
            self.set_tile(instance, s, t.x as u8, t.y as i32, t.z as i32, t.w as i32);
        }
    }

    /** Check if tile exists, if chunk is empty then default to false, do not create a new chunk */
    pub fn is_tile(&self, ix: i32, iy: i32, iz: i32) -> bool {
        match self.get_chunk(ix, iy, iz) {
            Some(c) => {
                let index = ((((ix.rem_euclid(CHUNK_SIZE) * CHUNK_SIZE)
                    + iy.rem_euclid(CHUNK_SIZE))
                    * CHUNK_SIZE)
                    + iz.rem_euclid(CHUNK_SIZE)) as usize;
                c.cells[index].0 > 0
            }
            _ => false,
        }
    }

    /** If a chunk exists */
    pub fn drop_chunk(&mut self, x: i32, y: i32, z: i32) -> Option<String> {
        let key = format!(
            "{}:{}:{}",
            x.div_euclid(CHUNK_SIZE),
            y.div_euclid(CHUNK_SIZE),
            z.div_euclid(CHUNK_SIZE)
        );
        match self.chunks.remove_entry(&key) {
            Some(pair) => Some(pair.0),
            None => None,
        }
    }

    pub fn _get_chunk_mut(&mut self, key: String) -> Option<&mut Chunk> {
        match self.chunks.entry(key.clone()) {
            Entry::Occupied(o) => Some(o.into_mut()),
            _ => None,
        }
    }

    pub fn get_chunk_mut_from_pos(&mut self, x: i32, y: i32, z: i32) -> &mut Chunk {
        let key = format!(
            "{}:{}:{}",
            x.div_euclid(CHUNK_SIZE),
            y.div_euclid(CHUNK_SIZE),
            z.div_euclid(CHUNK_SIZE)
        );
        // println!("hash chunk at {} {} {} {}",x,y,z,key);
        match self.chunks.entry(key.clone()) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let ix = x.div_euclid(CHUNK_SIZE) * CHUNK_SIZE;
                let iy = y.div_euclid(CHUNK_SIZE) * CHUNK_SIZE;
                let iz = z.div_euclid(CHUNK_SIZE) * CHUNK_SIZE;
                // println!("get new chunk {} {} {} {}", key, ix, iy, iz);
                v.insert(Chunk::new(key, ix, iy, iz))
            }
        }
    }
    pub fn get_chunk_from_pos(&self, x: i32, y: i32, z: i32) -> Option<&Chunk> {
        let key = format!(
            "{}:{}:{}",
            x.div_euclid(CHUNK_SIZE),
            y.div_euclid(CHUNK_SIZE),
            z.div_euclid(CHUNK_SIZE)
        );
        self.chunks.get(&key)
    }

    pub fn get_all_chunks(&self) -> std::collections::hash_map::Values<String, Chunk> {
        self.chunks.values()
    }

    /** get all chunks, big! Used by renderer, ideally it will only show visible chunks */
    pub fn get_all_chunks_mut(&mut self) -> std::collections::hash_map::ValuesMut<String, Chunk> {
        self.chunks.values_mut()
    }

    pub fn destroy_it_all(&mut self) {
        self.dropped = true;
        self.chunks.clear();
    }
}
pub struct Chunk {
    pub dirty: bool,
    pub cells: [(u32, u8); (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize],
    /** Unmodified position within world space, index postion would be position divided by 16 */
    pub pos: IVec3,
    pub key: String,
    // DEV `pub ind_pos` it might be worth storing the index position rather then dividing by 16 all the time?
}

impl Chunk {
    pub fn new(key: String, x: i32, y: i32, z: i32) -> Chunk {
        Chunk {
            dirty: false,
            cells: [(0, 0); (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize],
            pos: ivec3(x, y, z),
            key,
        }
    }

    pub fn clone(&self) -> Chunk {
        Chunk {
            dirty: true,
            cells: self.cells.clone(),
            pos: self.pos.clone(),
            key: self.key.clone(),
        }
    }

    pub fn zero_cells(&mut self) {
        self.cells = [(0, 0); (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize];
    }
}

pub struct LayerModel {
    pub chunks: FxHashMap<String, ChunkModel>,
}

impl LayerModel {
    pub fn new() -> LayerModel {
        let l = LayerModel {
            chunks: FxHashMap::default(),
        };
        // let x: i32 = 100;
        // let y: i32 = 100;
        // let z: i32 = 100;
        // let key = format!(
        //     "{}:{}:{}",
        //     x.div_euclid(16),
        //     y.div_euclid(16),
        //     z.div_euclid(16)
        // );
        // let mut c = ChunkModel::new(key.clone(), x, y, z);
        // c.cook(device);
        // l.chunks.insert(key.clone(), c);
        l
        // world.get_chunk_mut(100, 100, 100).cook(device);
    }

    pub fn destroy_it_all(&mut self) {
        self.chunks.clear();
    }
}

pub struct ChunkModel {
    pub vert_data: Vec<Vertex>,
    pub ind_data: Vec<u32>,
    pub buffers: Option<(Buffer, Buffer)>,
    pub instance_buffer: Buffer,
    pub pos: IVec3,
    pub key: String,
}

impl ChunkModel {
    pub fn new(device: &Device, key: String, x: i32, y: i32, z: i32) -> ChunkModel {
        let pos = ivec3(x, y, z);
        // println!("chunk pos {:?}", pos);
        // ChunkModel::c
        let t = ChunkModel::create_transform(pos);
        let instance_buffer = ChunkModel::create_buffer(device, t);

        ChunkModel {
            vert_data: vec![],
            ind_data: vec![],
            buffers: None,
            instance_buffer,
            pos,
            key,
        }
    }

    /* Piece together the chunk with either tilemap cubes, or gltf/glb model instances based on tile index number */
    pub fn build_chunk(
        &mut self,
        tex_map: &FxHashMap<u32, Vec4>,
        model_map: &FxHashMap<u32, Rc<Model>>,
        model_manager: &ModelManager,
        chunk: Chunk,
    ) {
        self.vert_data = vec![];
        self.ind_data = vec![];

        self.buffers = None;
        // let texture = "grid".to_string(); // grass_down
        // MARK change model

        // println!("got cells {}", chunk.cells.len());

        // we need to mutate teh chunk with vertex data, so we clone it's cell array to build our 3d grid with
        for (i, cell) in chunk.cells.clone().iter().enumerate() {
            if (*cell).0 > 0u32 {
                _add_tile_model(
                    tex_map,
                    model_map,
                    model_manager,
                    self,
                    (*cell).0,
                    (*cell).1,
                    ((i as i32 / (CHUNK_SIZE * CHUNK_SIZE)) % CHUNK_SIZE),
                    ((i as i32 / CHUNK_SIZE) % CHUNK_SIZE),
                    (i as i32 % CHUNK_SIZE),
                );
            }
        }
    }

    fn create_buffer(device: &Device, e: EntityUniforms) -> Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&[e]),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }
    fn create_transform(pos: IVec3) -> EntityUniforms {
        let offset = (pos.clone()).as_vec3();
        // + ivec3(-CHUNK_SIZE / 2, -CHUNK_SIZE / 2, -CHUNK_SIZE / 2).as_vec3();

        // println!("offset {:?}", offset);
        // let model =
        //     Mat4::from_scale_rotation_translation(vec3(16., 16., 16.), Quat::IDENTITY, offset);
        let model = Mat4::from_translation(offset.mul(16.));
        // let model = Mat4::IDENTITY;

        EntityUniforms {
            color: [0.; 4],
            uv_mod: [0., 0., 1., 1.],
            effects: [0; 4],
            model: model.to_cols_array_2d(),
        }
    }

    pub fn cook(&mut self, device: &Device) {
        let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Vertex Buffer"),
            contents: bytemuck::cast_slice(&self.vert_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Index Buffer"),
            contents: bytemuck::cast_slice(&self.ind_data),
            usage: wgpu::BufferUsages::INDEX,
        });
        // log(format!(
        //     "cooked with tile indicie data: {}",
        //     self.ind_data.len()
        // ));
        self.buffers = Some((vertex_buf, index_buf));
        // self.dirty = false;
    }

    pub fn is_empty(&self) -> bool {
        self.vert_data.len() < 3
    }
}

fn _add_tile_model(
    tex_map: &FxHashMap<u32, Vec4>,
    model_map: &FxHashMap<u32, Rc<Model>>,
    model_manager: &ModelManager,
    c: &mut ChunkModel,
    model_index: u32,
    meta: u8,
    ix: i32,
    iy: i32,
    iz: i32,
) {
    let current_count = c.vert_data.len() as u32;
    //println!("index bit adjustment {}", current_count);
    let offset = ivec3(ix as i32, iy as i32, iz as i32).mul(16) - ivec3(8, 8, 8);
    // + c.pos.clone()).mul(CHUNK_SIZE)
    //     + ivec3(-CHUNK_SIZE / 2, -CHUNK_SIZE / 2, -CHUNK_SIZE / 2);

    // println!(
    //     "model offset {} {} {} offset:{} key:{}",
    //     ix, iy, iz, offset, c.key
    // );

    let method = match meta {
        1 => |v: &mut Vertex| v.rotp90(),
        2 => |v: &mut Vertex| v.rotp180(),
        3 => |v: &mut Vertex| v.rotp270(),
        _ => |_: &mut Vertex| {},
    };

    let (mut verts, mut inds) = match model_map.get(&model_index) {
        Some(m) => {
            // let uv = crate::texture::get_tex_from_index(model_index);
            let modl = Rc::clone(m);
            let data = modl.data.as_ref().unwrap().clone();
            // print!("c{} {}", model_index, modl.name);
            // print!("🟢🟣uv{} {}", uv, modl.name);

            // log(format!("tile with {}",  meta));
            let verts = data
                .0
                .iter()
                .map(|v| {
                    let mut v2 = v.clone();
                    method(&mut v2);
                    // match meta {
                    //     1 => v2.rotp90(),
                    //     2 => v2.rotp180(),
                    //     3 => v2.rotp270(),
                    //     _ => {}
                    // }
                    v2.trans(offset);
                    // v2.texture(uv);

                    // v2.rotp90();
                    v2
                })
                .collect::<Vec<Vertex>>();

            let inds = data
                .1
                .iter()
                .map(|i| i.clone() + current_count)
                .collect::<Vec<u32>>();
            (verts, inds)
        }
        None => {
            let uv = match tex_map.get(&model_index) {
                Some(uv) => uv.clone(),
                _ => glam::vec4(1., 1., 0., 0.),
            };
            let cube = model_manager.cube_model();
            let data = cube.data.as_ref().unwrap().clone();

            // crate::model::create_plane(16, None, None)
            let verts = data
                .0
                .iter()
                .map(|v| {
                    let mut v2 = v.clone();
                    method(&mut v2);
                    v2.trans(offset);
                    v2.texture(uv);
                    v2
                })
                .collect::<Vec<Vertex>>();

            let inds = data
                .1
                .iter()
                .map(|i| i.clone() + current_count)
                .collect::<Vec<u32>>();
            (verts, inds)
        }
    };

    // let (verts, inds) = match crate::model::get_adjustable_model(model) {
    //     Some(m) => {
    //         // println!("🟢we got a cube model");
    //         let data = m.get().unwrap().data.as_ref().unwrap().clone();
    //         (data.0, data.1)
    //     }
    //     None => {
    //         //println!("🔴failed to locate cube model");
    //         crate::model::create_plane(16, None, None)
    //     }
    // };

    // 0 texture don't bother offseting the uv on the model
    // let mut verts2 = if texture < 2 {
    //     verts
    //         .iter()
    //         .map(|v| {
    //             let mut v2 = v.clone();
    //             v2.trans(offset.clone());
    //             v2
    //         })
    //         .collect::<Vec<Vertex>>()
    // } else {
    //     verts
    //         .iter()
    //         .map(|v| {
    //             let mut v2 = v.clone();
    //             v2.trans(offset.clone());
    //             v2.texture(uv);
    //             v2
    //         })
    //         .collect::<Vec<Vertex>>()
    // };

    // let inds2 = data
    //     .1
    //     .iter()
    //     .map(|i| *i + current_count)
    //     .collect::<Vec<u32>>();

    // let mut inds2 = inds
    //     .iter()
    //     .map(|i| i.clone() + current_count)
    //     .collect::<Vec<u32>>();

    c.vert_data.append(&mut verts);

    // let ind2 = i
    c.ind_data.append(&mut inds);
}

// fn default_tile_size() -> u16 {}
fn log(str: String) {
    crate::log::log(format!("🪴tile::{}", str));
    println!("🪴tile::{}", str);
}

/* TEST
fn test(ix: i32, iy: i32, iz: i32) {
    let i = ((((ix.rem_euclid(16) * 16) + iy.rem_euclid(16)) * 16) + iz.rem_euclid(16)) as usize;
    let nx = ((i / 256) % 16) as i32;
    let ny = ((i / 16) % 16) as i32;
    let nz = (i % 16) as i32;
    assert_eq!(ix.rem_euclid(16), nx);
    assert_eq!(iy.rem_euclid(16), ny);
    assert_eq!(iz.rem_euclid(16), nz);
}

fn main() {
    for i in -100..100 {
        for j in -100..100 {
            for k in -100..100 {
                test(i, j, k);
            }
        }
    }
}
*/

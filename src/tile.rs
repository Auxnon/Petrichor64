use rustc_hash::FxHashMap;
use std::{collections::hash_map::Entry, ops::Mul, rc::Rc};

use glam::{ivec3, IVec3, Mat4, Vec4};
#[cfg(feature = "headed")]
use wgpu::{util::DeviceExt, Buffer, Device};

#[cfg(feature = "headed")]
use crate::ent::EntityUniforms;
use crate::{
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
        let cell_type = if tile.len() == 0 {
            0
        } else {
            match instance.get_model_index(&tile) {
                Some(model_index) => model_index,
                _ => instance.get_tex_index(&tile),
            }
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
    pub fn get_tile(
        &self,
        instance: &WorldInstance,
        ix: i32,
        iy: i32,
        iz: i32,
    ) -> (Option<(String, u8)>) {
        let t = match self.get_chunk(ix, iy, iz) {
            Some(c) => {
                let index = ((((ix.rem_euclid(CHUNK_SIZE) * CHUNK_SIZE)
                    + iy.rem_euclid(CHUNK_SIZE))
                    * CHUNK_SIZE)
                    + iz.rem_euclid(CHUNK_SIZE)) as usize;
                c.cells[index]
            }
            _ => (0, 0),
        };

        if t.0 == 0 {
            None
        } else {
            match instance.get_model_by_index(t.0) {
                Some(m) => Some((m, t.1)),
                _ => match instance.get_tex_by_index(t.0) {
                    Some(tex) => Some((tex, t.1)),
                    _ => None,
                },
            }
        }
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
    /** Search at i position in d direction until tile of type is located, fail if search limit amount reached */
    pub fn first_tile(
        &self,
        instance: &WorldInstance,
        tile: Option<String>,
        mut ix: i32,
        mut iy: i32,
        mut iz: i32,
        dx: i32,
        dy: i32,
        dz: i32,
        limit: u32,
    ) -> Option<[i32; 3]> {
        if limit == 0 {
            return None;
        }
        if dx == 0 && dy == 0 && dz == 0 {
            return None;
        }
        let target = match tile {
            Some(t) => {
                if t.len() == 0 {
                    0
                } else {
                    match instance.get_model_index(&t) {
                        Some(model_index) => model_index,
                        _ => instance.get_tex_index(&t),
                    }
                }
            }
            _ => 0,
        };
        // let target = if tlen == 0 {
        //     0
        // } else {
        //     match instance.get_model_index(&tile) {
        //         Some(model_index) => model_index,
        //         _ => instance.get_tex_index(&tile),
        //     }
        // };
        let mut rx = ix.div_euclid(CHUNK_SIZE);
        let mut ry = iy.div_euclid(CHUNK_SIZE);
        let mut rz = iz.div_euclid(CHUNK_SIZE);
        let sx = rx * CHUNK_SIZE;
        let sy = ry * CHUNK_SIZE;
        let sz = rz * CHUNK_SIZE;

        let mut px = (ix - sx) as u16;
        let mut py = (iy - sy) as u16;
        let mut pz = (iz - sz) as u16;

        let mut i = 0;

        loop {
            if i > limit {
                return None;
            }
            let key = format!("{}:{}:{}", rx, ry, rz);
            match self.chunks.get(&key) {
                Some(c) => match c.first_tile(
                    target, px, py, pz, &mut ix, &mut iy, &mut iz, dx, dy, dz, &mut i, limit,
                ) {
                    (None, Some(t)) => {
                        // if we hit a chunk boundary
                        rx += t[0] as i32;
                        ry += t[1] as i32;
                        rz += t[2] as i32;
                        px = (ix - (rx * CHUNK_SIZE)) as u16;
                        py = (iy - (ry * CHUNK_SIZE)) as u16;
                        pz = (iz - (rz * CHUNK_SIZE)) as u16;
                    }
                    (Some(t), _) => {
                        return Some([t[0], t[1], t[2]]);
                    }
                    (None, None) => return None,
                },
                None => {
                    if target == 0 {
                        return Some([ix, iy, iz]);
                    }

                    rx += dx;
                    ry += dy;
                    rz += dz;
                    if dx != 0 {
                        ix += dx * CHUNK_SIZE;
                    }
                    if dy != 0 {
                        iy += dy * CHUNK_SIZE;
                    }
                    if dz != 0 {
                        iz += dz * CHUNK_SIZE;
                    }
                    i += CHUNK_SIZE as u32;
                }
            };
        }
        // loop
        // match self.get_chunk(ix, iy, iz) {
        //     Some(c) => {
        //         let index = ((((ix.rem_euclid(CHUNK_SIZE) * CHUNK_SIZE)
        //             + iy.rem_euclid(CHUNK_SIZE))
        //             * CHUNK_SIZE)
        //             + iz.rem_euclid(CHUNK_SIZE)) as usize;
        //         if c.cells[index].0 > 0 {
        //             Some(c.cells[index])
        //         } else {
        //             None
        //         }
        //     }
        //     _ => None,
        // }
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

    pub fn stats(&self) {
        println!("--global_layer -> global_chunks#:{} ", self.chunks.len());
        self.chunks.iter().enumerate().for_each(|(i, (k, v))| {
            println!("---chunk {} [{}] -> p:{}", i, k, v.pos);
        });
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

    /** Chunk based first tile linear search. Do not pass a 0,0,0 direction or we're just wasting time */
    pub fn first_tile(
        &self,
        target: u32,
        x: u16,
        y: u16,
        z: u16,
        ox: &mut i32,
        oy: &mut i32,
        oz: &mut i32,
        dx: i32,
        dy: i32,
        dz: i32,
        current_iterator: &mut u32,
        limit: u32,
    ) -> (Option<[i32; 3]>, Option<[i8; 3]>) {
        let i = current_iterator;
        let mut ix = x as i32;
        let mut iy = y as i32;
        let mut iz = z as i32;

        loop {
            if *i >= limit {
                return (None, None);
            }
            *i += 1;
            ix += dx;
            *ox += dx;
            if ix < 0 {
                return (None, Some([-1, 0, 0]));
            }
            if ix >= CHUNK_SIZE as i32 {
                return (None, Some([1, 0, 0]));
            }
            iy += dy;
            *oy += dy;
            if iy < 0 {
                return (None, Some([0, -1, 0]));
            }
            if iy >= CHUNK_SIZE as i32 {
                return (None, Some([0, 1, 0]));
            }
            iz += dz;
            *oz += dz;
            if iz < 0 {
                return (None, Some([0, 0, -1]));
            }
            if iz >= CHUNK_SIZE as i32 {
                return (None, Some([0, 0, 1]));
            }

            let index = ((((ix * CHUNK_SIZE as i32) + iy) * CHUNK_SIZE as i32) + iz) as usize;
            if self.cells[index].0 == target {
                return (Some([*ox, *oy, *oz]), None);
            }
        }
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
    #[cfg(feature = "headed")]
    pub buffers: Option<(Buffer, Buffer)>,
    #[cfg(feature = "headed")]
    pub instance_buffer: Buffer,
    pub pos: IVec3,
    pub key: String,
}

impl ChunkModel {
    pub fn new(
        #[cfg(feature = "headed")] device: &Device,
        key: String,
        x: i32,
        y: i32,
        z: i32,
    ) -> ChunkModel {
        let pos = ivec3(x, y, z);
        #[cfg(feature = "headed")]
        let t = ChunkModel::create_transform(pos);
        #[cfg(feature = "headed")]
        let instance_buffer = ChunkModel::create_buffer(
            #[cfg(feature = "headed")]
            device,
            t,
        );

        ChunkModel {
            vert_data: vec![],
            ind_data: vec![],
            #[cfg(feature = "headed")]
            buffers: None,
            #[cfg(feature = "headed")]
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
        #[cfg(feature = "headed")]
        {
            self.buffers = None;
        }
        self.vert_data = vec![];
        self.ind_data = vec![];

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
                    (i as i32 / (CHUNK_SIZE * CHUNK_SIZE)) % CHUNK_SIZE,
                    (i as i32 / CHUNK_SIZE) % CHUNK_SIZE,
                    i as i32 % CHUNK_SIZE,
                );
            }
        }
    }

    #[cfg(feature = "headed")]
    fn create_buffer(device: &Device, e: EntityUniforms) -> Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&[e]),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }
    #[cfg(feature = "headed")]
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
            effects: [0.; 4],
            model: model.to_cols_array_2d(),
        }
    }

    #[cfg(feature = "headed")]
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

    c.vert_data.append(&mut verts);

    // let ind2 = i
    c.ind_data.append(&mut inds);
}

// fn default_tile_size() -> u16 {}
// fn log(str: String) {
//     crate::log::log(format!("🪴tile::{}", str));
//     println!("🪴tile::{}", str);
// }

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

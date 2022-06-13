use ron::de::from_reader;
use ron::Map;
use rustc_hash::FxHashMap;
use serde::Deserialize;
use std::collections::hash_map::Entry;

use glam::{ivec3, vec3, DVec4, IVec3, Vec4};
use rand::Rng;
use wgpu::{util::DeviceExt, Buffer, Device};

use crate::model::Vertex;

/*
texture index of 0 is air and not rendered
texture index of 1 is "plain" and unmodifies texture
2..N is any texture index in the atlas
*/
pub struct World {
    layer: Layer,
}

impl World {
    pub fn new(device: &Device) -> World {
        let mut world = World {
            layer: Layer::new(),
        };

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

        world.get_chunk_mut(100, 100, 100).cook(device);
        world
    }

    pub fn get_chunk_mut(&mut self, x: i32, y: i32, z: i32) -> &mut Chunk {
        self.layer.get_chunk_mut_from_pos(x, y, z)
    }
    pub fn get_all_chunks(&self) -> std::collections::hash_map::Values<String, Chunk> {
        self.layer.get_all_chunks()
    }

    // pub fn build_chunk_from_pos(&mut self, ix: i32, iy: i32, iz: i32) {
    //     let c = self.get_chunk_mut(ix, iy, iz);
    //     self.build_chunk2(c);
    // }

    pub fn build_dirty_chunks(&mut self, device: &Device) {
        let mut d = vec![];
        for c in self.layer.get_all_chunks() {
            if c.dirty {
                d.push(c.key.clone());
            }
        }
        println!("we found {} dirty chunk(s)", d.len());
        for k in d {
            match self.layer.get_chunk_mut(k) {
                Some(c) => {
                    c.build_chunk();
                    c.cook(device);
                }
                _ => {}
            }
        }
    }

    pub fn set_tile(&mut self, tile: &String, ix: i32, iy: i32, iz: i32) {
        let cell_type = match crate::model::get_model_index(&tile) {
            Some(model_index) => model_index,
            _ => crate::texture::get_tex_index(&tile),
        };

        // let uv = crate::texture::get_tex_index(&tile);

        let mut c = self.get_chunk_mut(ix, iy, iz);
        let index =
            ((((ix.rem_euclid(16) * 16) + iy.rem_euclid(16)) * 16) + iz.rem_euclid(16)) as usize;
        // let index = ((((ix / 16).rem_euclid(16)) * 16 + ((iy / 16).rem_euclid(16))) * 16
        //     + ((iz / 16).rem_euclid(16))) as usize;
        c.cells[index] = cell_type;
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

    pub fn set_tile_from_buffer(&mut self, buffer: &Vec<(String, Vec4)>) {
        for (s, t) in buffer {
            self.set_tile(s, t.y as i32, t.z as i32, t.w as i32);
        }
    }
    pub fn destroy_it_all(&mut self) {
        self.layer.destroy_it_all();
    }

    // pub fn add_tile_model(&mut self, texture: &String, model: &String, ix: i32, iy: i32, iz: i32) {
    //     let c = self.get_chunk_mut(ix, iy, iz);
    //     _add_tile_model(c, texture, model, ix, iy, iz);
    // }
}

pub struct Layer {
    chunks: FxHashMap<String, Chunk>,
}

impl Layer {
    pub fn new() -> Layer {
        Layer {
            chunks: FxHashMap::default(), // ![Chunk::new()],
        }
    }

    pub fn get_chunk_mut(&mut self, key: String) -> Option<&mut Chunk> {
        match self.chunks.entry(key.clone()) {
            Entry::Occupied(o) => Some(o.into_mut()),
            Entry::Vacant(v) => None,
        }
    }

    pub fn get_chunk_mut_from_pos(&mut self, x: i32, y: i32, z: i32) -> &mut Chunk {
        let key = format!(
            "{}:{}:{}",
            x.div_euclid(16),
            y.div_euclid(16),
            z.div_euclid(16)
        );
        // println!("hash chunk at {} {} {} {}",x,y,z,key);
        match self.chunks.entry(key.clone()) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => {
                let ix = x.div_euclid(16) * 16;
                let iy = y.div_euclid(16) * 16;
                let iz = z.div_euclid(16) * 16;
                // println!("get new chunk {} {} {} {}", key, ix, iy, iz);
                v.insert(Chunk::new(key, ix, iy, iz))
            }
        }
    }

    pub fn get_all_chunks(&self) -> std::collections::hash_map::Values<String, Chunk> {
        self.chunks.values()
    }

    pub fn get_all_chunks_mut(&mut self) -> std::collections::hash_map::ValuesMut<String, Chunk> {
        self.chunks.values_mut()
    }

    pub fn destroy_it_all(&mut self) {
        self.chunks.clear();
    }
}
pub struct Chunk {
    pub dirty: bool,
    pub vert_data: Vec<Vertex>,
    pub ind_data: Vec<u32>,
    pub buffers: Option<(Buffer, Buffer)>,
    pub cells: [u32; 16 * 16 * 16],
    /** Unmodified position within world space, index postion would be position divided by 16 */
    pub pos: IVec3,
    pub key: String,
    // DEV `pub ind_pos` it might be worth storing the index position rather then dividing by 16 all the time?
}

impl Chunk {
    pub fn new(key: String, x: i32, y: i32, z: i32) -> Chunk {
        Chunk {
            dirty: false,
            vert_data: vec![],
            ind_data: vec![],
            buffers: None,
            cells: [0; 16 * 16 * 16],
            pos: ivec3(x, y, z),
            key,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.vert_data.len() < 3
    }

    fn zero_cells(&mut self) {
        self.cells = [0; 16 * 16 * 16];
    }

    pub fn build_chunk(&mut self) {
        self.vert_data = vec![];
        self.ind_data = vec![];
        self.buffers = None;
        // let texture = "grid".to_string(); // grass_down
        // MARK change model

        // println!("got cells {}", self.cells.len());

        // we need to mutate teh chunk with vertex data, so we clone it's cell array to build our 3d grid with
        for (i, cell) in self.cells.clone().iter().enumerate() {
            if *cell > 0u32 {
                _add_tile_model(
                    self,
                    *cell,
                    ((i / 256) % 16) as i32,
                    ((i / 16) % 16) as i32,
                    (i % 16) as i32,
                );
            }
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
        // println!(
        //     "tiles::cooked with tile indicie data: {}",
        //     self.ind_data.len()
        // );
        self.buffers = Some((vertex_buf, index_buf));
        self.dirty = false;
    }
}

fn _add_tile_model(c: &mut Chunk, model_index: u32, ix: i32, iy: i32, iz: i32) {
    let current_count = c.vert_data.len() as u32;
    //println!("index bit adjustment {}", current_count);
    let offset = ivec3(ix as i32, iy as i32, iz as i32) + c.pos.clone();
    // println!(
    //     "model offset {} {} {} c.pos:{} offset:{} key:{}",
    //     ix, iy, iz, c.pos, offset, c.key
    // );

    let (mut verts, mut inds) = match crate::model::get_model_from_index(model_index) {
        Some(m) => {
            let data = m.get().unwrap().data.as_ref().unwrap().clone();

            let verts = data
                .0
                .iter()
                .map(|v| {
                    let mut v2 = v.clone();
                    v2.trans(offset.clone());
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
            let uv = crate::texture::get_tex_from_index(model_index);
            let cube = crate::model::cube_model();
            let data = cube.get().unwrap().data.as_ref().unwrap().clone();
            // println!("🟢 loaded cube with text {}", uv);
            // crate::model::create_plane(16, None, None)
            let verts = data
                .0
                .iter()
                .map(|v| {
                    let mut v2 = v.clone();
                    v2.trans(offset.clone());
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

/** load file as a ron template for tile */
pub fn load_ron(path: &String) -> Option<TileTemplate> {
    match std::fs::File::open(path) {
        Ok(f) => match from_reader(f) {
            Ok(x) => Some(x),
            Err(e) => {
                log(format!("problem with template {}", e));
                None
            }
        },
        _ => None,
    }
}

/** interpret string as a ron template for tile */
pub fn interpret_ron(s: &String) -> Option<TileTemplate> {
    match ron::from_str(s) {
        Ok(x) => Some(x),
        Err(e) => {
            log(format!("problem with template {}", e));
            //std::process::exit(1);
            None
        }
    }
}

#[derive(Default, Debug, Deserialize)]
pub struct TileTemplate {
    #[serde(default = "default_tile_name")]
    pub name: String,
    #[serde(default)]
    pub tiles: std::collections::HashMap<u32, String>,
    #[serde(default = "default_tile_size")]
    pub size: u32,
}

fn default_tile_name() -> String {
    "".to_string()
}

fn default_tile_size() -> u32 {
    0 as u32
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

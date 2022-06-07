use std::collections::hash_map::Entry;

use rustc_hash::FxHashMap;

use glam::{ivec3, vec3, DVec4, IVec3, Vec4};
use rand::Rng;
use wgpu::{util::DeviceExt, Buffer, Device};

use crate::model::Vertex;

pub struct World {
    layer: Layer,
}
impl World {
    pub fn new(device: &Device) -> World {
        let mut world = World {
            layer: Layer::new(),
        };

        // add_tile(&mut w, "grass_down".to_string(), 0, 0, 0);
        // add_tile(&mut w, "grass_down".to_string(), 0, 1, 0);
        // add_tile(&mut w, "grass_down".to_string(), 1, 1, 0);
        let path = [[0, 0, 0], [0, 0, 0], [0, 0, 0]];
        let h = path.len();
        let w = path[0].len();
        let mut hash = FxHashMap::default();
        hash.insert(4, 3);
        hash.insert(2, 33);
        hash.insert(13, 52);
        hash.insert(1, 25);

        // hash.insert(2, 28);
        // hash.insert(10, 29);
        // hash.insert(8, 31);
        // hash.insert(4, 25);
        // hash.insert(11, 19); //27
        // hash.insert(15, 33);

        for yi in 0..path.len() {
            let row = path[yi];
            for xi in 0..row.len() {
                let c = row[xi];
                let x = xi as i32 * 16;
                let y = yi as i32 * -16;
                if c == 1 {
                    let u = if yi > 0 { path[yi - 1][xi] } else { 0 };
                    let d = if yi < h - 1 { path[yi + 1][xi] } else { 0 };
                    let l = if xi > 0 { path[yi][xi - 1] } else { 0 };
                    let r = if xi < w - 1 { path[yi][xi + 1] } else { 0 };
                    let bit = u | r << 1 | d << 2 | l << 3;
                    let h = match hash.get(&bit) {
                        Some(n) => n.clone(),
                        _ => 34,
                    };
                    println!("x{} y{} b{}", xi, yi, bit);
                    world.set_tile(format!("map{}", h), x, y, 0);
                } else {
                    world.set_tile(format!("map{}", 44), x, y, 0); //36
                }

                //if c == 1 {

                //} else {
                // add_tile(&mut w, format!("grass{}", 5), x, y, 0);
                //}
            }
        }
        // let mut rn = rand::thread_rng();
        // let mut ind = 0;
        // for i in 0..1000 {
        //     //1000000
        //     let x = ((i as f32 * 20.).cos() * 48.) as i32;
        //     let y = ((i as f32 * 20.).sin() * 48.) as i32; //rn.gen_range(0..128) - 64;
        //     let model = format!("grass{}", ind); //"grass_down".to_string();

        //     add_tile(&mut w, model, x, y, -i * 2);
        //     ind += 1;
        //     if ind >= 16 {
        //         ind = 0;
        //     }
        // }
        for i in 0..16 {
            for j in 0..16 {
                world.set_tile(format!("fatty"), (i - 8) * 16, (j - 8) * 16, -32 * 3)
            }
        }
        for i in 0..16 {
            for j in 0..16 {
                world.set_tile(format!("grid"), 16 * 16, (i - 8) * 16, (j - 8) * 16)
            }
        }
        world.set_tile(format!("grid"), 0, 0, 16 * 0);

        world.get_chunk_mut(0, 0, 0).cook(device);
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

    pub fn set_tile(&mut self, tile: String, ix: i32, iy: i32, iz: i32) {
        let t = match tile.as_str() {
            "grid" => 1,
            "grass" => 2,
            _ => 0,
        };
        let mut c = self.get_chunk_mut(ix, iy, iz);
        let index =
            ((((ix.rem_euclid(16) * 16) + iy.rem_euclid(16)) * 16) + iz.rem_euclid(16)) as usize;
        // let index = ((((ix / 16).rem_euclid(16)) * 16 + ((iy / 16).rem_euclid(16))) * 16
        //     + ((iz / 16).rem_euclid(16))) as usize;
        c.cells[index] = t;
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

    pub fn set_tile_from_buffer(&mut self, buffer: &Vec<Vec4>) {
        for t in buffer {
            self.set_tile("grid".to_string(), t.y as i32, t.z as i32, t.w as i32);
        }
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
        let key = format!("{}-{}-{}", x / 16, y / 16, z / 16);
        match self.chunks.entry(key.clone()) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(Chunk::new(key, x, y, z)),
        }
    }
    pub fn get_all_chunks(&self) -> std::collections::hash_map::Values<String, Chunk> {
        self.chunks.values()
    }
    pub fn get_all_chunks_mut(&mut self) -> std::collections::hash_map::ValuesMut<String, Chunk> {
        self.chunks.values_mut()
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
        let texture = "grid".to_string(); // grass_down
        let model = "cube".to_string();
        // println!("got cells {}", self.cells.len());

        // we need to mutate teh chunk with vertex data, so we clone it's cell array to build our 3d grid with
        for (i, cell) in self.cells.clone().iter().enumerate() {
            if *cell > 0u32 {
                _add_tile_model(
                    self,
                    &texture,
                    &model,
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

fn _add_tile_model(c: &mut Chunk, texture: &String, model: &String, ix: i32, iy: i32, iz: i32) {
    let current_count = c.vert_data.len() as u32;
    //println!("index bit adjustment {}", current_count);
    let offset = ivec3(ix as i32, iy as i32, iz as i32) + c.pos;

    let uv = crate::texture::get_tex(texture);

    let (verts, inds) = match crate::model::get_adjustable_model(model) {
        Some(m) => {
            // println!("🟢we got a cube model");
            let data = m.get().unwrap().data.as_ref().unwrap().clone();
            (data.0, data.1)
        }
        None => {
            //println!("🔴failed to locate cube model");
            crate::model::create_plane(16, None, None)
        }
    };

    // println!("model is {} {} {}", ix, iy, iz);
    let mut verts2 = verts
        .iter()
        .map(|v| {
            let mut v2 = v.clone();
            v2.trans(offset);
            v2.texture(uv);
            v2
        })
        .collect::<Vec<Vertex>>();

    // let inds2 = data
    //     .1
    //     .iter()
    //     .map(|i| *i + current_count)
    //     .collect::<Vec<u32>>();

    let mut inds2 = inds
        .iter()
        .map(|i| i.clone() + current_count)
        .collect::<Vec<u32>>();

    c.vert_data.append(&mut verts2);

    // let ind2 = i
    c.ind_data.append(&mut inds2);
}

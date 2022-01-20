use std::collections::HashMap;

use glam::{ivec3, vec3};
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
        let path = [[0, 1, 0], [1, 1, 0], [0, 1, 0]];
        let h = path.len();
        let w = path[0].len();
        let mut hash = HashMap::new();
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
                    add_tile(&mut world, format!("map{}", h), x, y, 0);
                } else {
                    add_tile(&mut world, format!("map{}", 44), x, y, 0); //36
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

        world.get_tile_mut(0, 0).cook(device);
        world
    }
    pub fn get_tile_mut(&mut self, x: i32, y: i32) -> &mut Chunk {
        self.layer.get_tile_mut(x, y)
    }
}
pub struct Layer {
    chunks: Vec<Chunk>,
}
impl Layer {
    pub fn new() -> Layer {
        Layer {
            chunks: vec![Chunk::new()],
        }
    }
    pub fn get_tile_mut(&mut self, x: i32, y: i32) -> &mut Chunk {
        self.chunks.get_mut(0).unwrap()
    }
}
pub struct Chunk {
    pub vert_data: Vec<Vertex>,
    pub ind_data: Vec<u32>,
    pub buffers: Option<(Buffer, Buffer)>,
}
impl Chunk {
    pub fn new() -> Chunk {
        Chunk {
            vert_data: vec![],
            ind_data: vec![],
            buffers: None,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.vert_data.len() < 3
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
        println!("cooked at {}", self.ind_data.len());
        self.buffers = Some((vertex_buf, index_buf));
    }
}

pub fn add_tile(mut world: &mut World, model: String, ix: i32, iy: i32, iz: i32) {
    let mut c = world.get_tile_mut(ix, iy);
    let current_count = c.vert_data.len() as u32;
    //println!("index bit adjustment {}", current_count);
    let offset = ivec3(ix as i32, iy as i32, iz as i32);

    let uv = crate::texture::get_tex(model);

    let (verts, inds) = match crate::model::get_adjustable_model(&"plane".to_string()) {
        Some(m) => {
            let data = m.get().unwrap().data.as_ref().unwrap().clone();
            (data.0, data.1)
        }
        None => crate::model::create_plane(16, None, None),
    };

    //println!("model is {} {} {} {}", uv.x, uv.y, uv.z, uv.w);
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

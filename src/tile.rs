use rand::Rng;
use wgpu::{util::DeviceExt, Buffer, Device};

use crate::model::Vertex;

pub struct World {
    layer: Layer,
}
impl World {
    pub fn new(device: &Device) -> World {
        let mut w = World {
            layer: Layer::new(),
        };

        // add_tile(&mut w, "grass_down".to_string(), 0, 0, 0);
        // add_tile(&mut w, "grass_down".to_string(), 0, 1, 0);
        // add_tile(&mut w, "grass_down".to_string(), 1, 1, 0);
        let mut rn = rand::thread_rng();
        for i in 0..10000 {
            //1000000
            let x = ((i as f32 * 20.).cos() * 48.) as i32;
            let y = ((i as f32 * 20.).sin() * 48.) as i32; //rn.gen_range(0..128) - 64;
            add_tile(&mut w, "grass_down".to_string(), x, y, -i * 2);
        }

        w.get_tile_mut(0, 0).cook(device);
        w
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
    let offset = cgmath::vec3(ix as i16, iy as i16, iz as i16);

    let (mut verts, mut inds) = match crate::model::get_adjustable_model(&"plane".to_string()) {
        Some(m) => {
            let data = m.get().unwrap().data.as_ref().unwrap();
            let uv = crate::texture::get_tex("grass12".to_string());
            let vert = data
                .0
                .iter()
                .map(|v| {
                    let mut v2 = v.clone();
                    v2.trans(offset);
                    v2.texture(uv);
                    v2
                })
                .collect::<Vec<Vertex>>();

            let inds2 = data
                .1
                .iter()
                .map(|i| *i + current_count)
                .collect::<Vec<u32>>();

            (vert, inds2)
        }
        None => {
            let (verts, inds) = crate::model::create_plane(
                16,
                Some(offset),
                Some(crate::texture::get_tex("grass1".to_string())),
            );
            let inds2 = inds
                .iter()
                .map(|i| *i + current_count)
                .collect::<Vec<u32>>();
            (verts, inds2)
        }
    };

    c.vert_data.append(&mut verts);

    // let ind2 = i
    c.ind_data.append(&mut inds);
}

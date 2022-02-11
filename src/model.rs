use bytemuck::{Pod, Zeroable};
use glam::{ivec3, vec3, vec4, IVec3, Vec3, Vec4};
use gltf::{image::Data as ImageData, json::extensions::mesh, Texture};
use itertools::izip;
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use std::{collections::HashMap, path::Path, sync::Arc};
use wgpu::{util::DeviceExt, Device};

lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref cube: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
    pub static ref plane: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
    pub static ref custom: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
    pub static ref dictionary:Mutex<HashMap<String,Arc<OnceCell<Model>> >> =Mutex::new(HashMap::new());
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    _pos: [i16; 4],
    _normal: [i8; 4],
    _tex: [f32; 2],
}
impl Vertex {
    pub fn trans(&mut self, pos: IVec3) {
        self._pos[0] += pos.x as i16;
        self._pos[1] += pos.y as i16;
        self._pos[2] += pos.z as i16;
    }
    pub fn texture(&mut self, uv: Vec4) {
        self._tex[0] = (self._tex[0] * uv.z) + uv.x;
        self._tex[1] = (self._tex[1] * uv.w) + uv.y;
    }
}

fn vertex(pos: [i16; 3], nor: [i8; 3], tex: [f32; 2]) -> Vertex {
    Vertex {
        _pos: [pos[0], pos[1], pos[2], 1],
        _normal: [nor[0], nor[1], nor[2], 0],
        _tex: [tex[0], tex[1]],
    }
}

fn vertexx(pos: [f32; 3], nor: [i8; 3], tex: [f32; 2]) -> Vertex {
    Vertex {
        _pos: [
            ((pos[0]) * 16.) as i16,
            ((pos[1]) * 16.) as i16,
            ((pos[2]) * 16.) as i16,
            1,
        ],
        _normal: [nor[0], nor[1], nor[2], 0],
        _tex: [tex[0], tex[1]],
    }
}

pub fn create_cube(i: i16) -> (Vec<Vertex>, Vec<u32>) {
    let vertex_data = [
        // top (0, 0, 1)
        vertex([-i, -i, i], [0, 0, 1], [0., 1.]),
        vertex([i, -i, i], [0, 0, 1], [1., 1.]),
        vertex([i, i, i], [0, 0, 1], [1., 0.]),
        vertex([-i, i, i], [0, 0, 1], [0., 0.]),
        // bottom (0, 0, -1)
        vertex([-i, i, -i], [0, 0, -1], [0., 0.]),
        vertex([i, i, -i], [0, 0, -1], [1., 0.]),
        vertex([i, -i, -i], [0, 0, -1], [1., 1.]),
        vertex([-i, -i, -i], [0, 0, -1], [0., 1.]),
        // right (1, 0, 0)
        vertex([i, -i, -i], [1, 0, 0], [0., 0.]),
        vertex([i, i, -i], [1, 0, 0], [1., 0.]),
        vertex([i, i, i], [1, 0, 0], [1., 1.]),
        vertex([i, -i, i], [1, 0, 0], [0., 1.]),
        // left (-1, 0, 0)
        vertex([-i, -i, i], [-1, 0, 0], [0., 0.]),
        vertex([-i, i, i], [-1, 0, 0], [1., 0.]),
        vertex([-i, i, -i], [-1, 0, 0], [1., 1.]),
        vertex([-i, -i, -i], [-1, 0, 0], [0., 1.]),
        // front (0, 1, 0)
        vertex([i, i, -i], [0, 1, 0], [0., 0.]),
        vertex([-i, i, -i], [0, 1, 0], [1., 0.]),
        vertex([-i, i, i], [0, 1, 0], [1., 1.]),
        vertex([i, i, i], [0, 1, 0], [0., 1.]),
        // back (0, -1, 0)
        vertex([i, -i, i], [0, -1, 0], [0., 0.]),
        vertex([-i, -i, i], [0, -1, 0], [1., 0.]),
        vertex([-i, -i, -i], [0, -1, 0], [1., 1.]),
        vertex([i, -i, -i], [0, -1, 0], [0., 1.]),
    ];

    let index_data: &[u32] = &[
        0, 1, 2, 2, 3, 0, // top
        4, 5, 6, 6, 7, 4, // bottom
        8, 9, 10, 10, 11, 8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    (vertex_data.to_vec(), index_data.to_vec())
}

pub fn create_plane(size: i16, offset: Option<IVec3>, uv: Option<Vec4>) -> (Vec<Vertex>, Vec<u32>) {
    let v: Vec4 = match uv {
        Some(v2) => v2,
        None => vec4(0., 0., 1., 1.),
    };
    let o: IVec3 = match offset {
        Some(o) => o,
        None => ivec3(0, 0, 0),
    };
    let ex = 1. * v.z + v.x;
    let sx = v.x;
    let ey = 1. * v.w + v.y;
    let sy = v.y;

    let vertex_data = [
        vertex(
            [o.x as i16 + size, o.y as i16, o.z as i16],
            [0, 0, 1],
            [ex, ey],
        ),
        vertex(
            [o.x as i16 + size, o.y as i16 + size, o.z as i16],
            [0, 0, 1],
            [ex, sy],
        ),
        vertex([o.x as i16, o.y as i16, o.z as i16], [0, 0, 1], [sx, ey]),
        vertex(
            [o.x as i16, o.y as i16 + size, o.z as i16],
            [0, 0, 1],
            [sx, sy],
        ),
    ];

    let index_data: &[u32] = &[0, 1, 2, 2, 1, 3];

    (vertex_data.to_vec(), index_data.to_vec())
}

pub fn build_model(device: &Device, str: String, verts: &Vec<Vertex>, inds: &Vec<u16>) -> Model {
    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Chunk Vertex Buffer"),
        contents: bytemuck::cast_slice(verts),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Chunk Index Buffer"),
        contents: bytemuck::cast_slice(inds),
        usage: wgpu::BufferUsages::INDEX,
    });
    Model {
        vertex_buf,
        index_buf,
        index_format: wgpu::IndexFormat::Uint32,
        index_count: inds.len(),
        data: None,
    }
}

pub fn init(device: &Device) {
    let (plane_vertex_data, plane_index_data) = create_plane(16, Some(ivec3(-8, 0, 0)), None);
    //device.create_buffer_init(desc)
    let plane_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Plane Vertex Buffer"),
        contents: bytemuck::cast_slice(&plane_vertex_data),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let plane_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Plane Index Buffer"),
        contents: bytemuck::cast_slice(&plane_index_data),
        usage: wgpu::BufferUsages::INDEX,
    });

    let planeModel = Model {
        vertex_buf: plane_vertex_buf,
        index_buf: plane_index_buf,
        index_format: wgpu::IndexFormat::Uint32,
        index_count: plane_index_data.len(),
        data: None,
    };
    plane.get_or_init(|| planeModel);

    let (cube_vertex_data, cube_index_data) = create_cube(8);
    let cube_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Cubes Vertex Buffer"),
        contents: bytemuck::cast_slice(&cube_vertex_data),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let cube_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Cubes Index Buffer"),
        contents: bytemuck::cast_slice(&cube_index_data),
        usage: wgpu::BufferUsages::INDEX,
    });
    let cubeModel = Model {
        vertex_buf: cube_vertex_buf,
        index_buf: cube_index_buf,
        index_format: wgpu::IndexFormat::Uint32,
        index_count: cube_index_data.len(),
        data: Some((cube_vertex_data, cube_index_data)),
    };
    cube.get_or_init(|| cubeModel);
    let d = dictionary
        .lock()
        .insert("cube".to_string(), Arc::clone(&cube));
}
pub fn cube_model() -> Arc<OnceCell<Model>> {
    Arc::clone(&cube)
}
pub fn plane_model() -> Arc<OnceCell<Model>> {
    Arc::clone(&plane)
}

pub fn get_model(str: &String) -> Arc<OnceCell<Model>> {
    if str == "plane" {
        return plane_model();
    }

    match dictionary.lock().get(str) {
        Some(model) => Arc::clone(model),
        None => cube_model(),
    }
    //Arc::clone(&custom)
}
pub fn get_adjustable_model(str: &String) -> Option<Arc<OnceCell<Model>>> {
    match dictionary.lock().get(str) {
        Some(model) => {
            let m = model.get();

            if m.is_some() {
                if m.unwrap().data.is_some() {
                    return Some(Arc::clone(model));
                } else {
                    log(format!("missing model data field for {}", str));
                }
            }
            return None;
        }
        None => None,
    }
}
pub struct Model {
    pub vertex_buf: wgpu::Buffer,
    pub index_buf: wgpu::Buffer,
    pub index_format: wgpu::IndexFormat,
    pub index_count: usize,
    pub data: Option<(Vec<Vertex>, Vec<u32>)>,
}

pub fn load(str: &String, device: &Device) {
    let bits = str.split(".").collect::<Vec<_>>();
    let name = bits.get(0).unwrap();
    let target = format!("assets/{}", str);
    log(target.to_string());
    let (nodes, buffers, image_data) = gltf::import(target).unwrap();
    //let mut meshes: Vec<Mesh> = vec![];
    //let im1 = image_data.get(0).unwrap();

    crate::texture::load_tex_from_img(str.to_string(), &image_data);

    //let tex = Texture2D::from_rgba8(im1.width as u16, im1.height as u16, &im1.pixels);
    //tex.set_filter(FilterMode::Nearest);
    let mut first_instance = true;
    for mesh in nodes.meshes() {
        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let verts_interleaved = izip!(
                reader.read_positions().unwrap(),
                //reader.read_normals().unwrap(),
                //reader.read_colors(0).unwrap().into_rgb_f32().into_iter(),
                reader.read_tex_coords(0).unwrap().into_f32(),
                //reader.read_indices().unwrap()
            );

            let vertices = verts_interleaved
                .map(|(pos, uv)|
                    // position: Vec3::from(pos),
                    // uv: Vec2::from(uv),
                    // color: WHITE,
                    vertexx(pos,[0,0,0],uv))
                .collect::<Vec<Vertex>>();

            if let Some(inds) = reader.read_indices() {
                let indices = inds.into_u32().map(|u| u as u32).collect::<Vec<u32>>();

                // let mesh = macroquad::models::Mesh {
                //     vertices,
                //     indices,
                //     texture: Some(tex),
                // };

                //             let index_data: &[u16] = &[0, 1, 2, 2, 1, 3];

                // (vertex_data.to_vec(), index_data.to_vec())

                //device.create_buffer_init(desc)
                let mesh_vertex_buf =
                    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some(&format!("{}{}", str, " Mesh Vertex Buffer")),
                        contents: bytemuck::cast_slice(&vertices),
                        usage: wgpu::BufferUsages::VERTEX,
                    });

                let mesh_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{}{}", str, " Mesh Index Buffer")),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });
                println!(
                    "Load mesh {} with {} verts",
                    str.to_string(),
                    vertices.len()
                );
                println!("ind #{} verts #{}", indices.len(), vertices.len());

                let model = Model {
                    vertex_buf: mesh_vertex_buf,
                    index_buf: mesh_index_buf,
                    index_format: wgpu::IndexFormat::Uint32,
                    index_count: indices.len(),
                    data: Some((vertices, indices)),
                };

                if first_instance {
                    first_instance = false;
                    let once = OnceCell::new();
                    once.get_or_init(|| model);
                    let i = Arc::new(once);
                    dictionary.lock().insert(name.to_string(), i);
                    log(format!("populated mesh {}", name));
                    //custom.get_or_init(|| customModel);
                }

                //rand::srand(6);

                //mat.mul_vec4(other)
                //meshes.push(mesh);
            };
        }
    }
    // match custom.get() {
    //     Some(m) => println!("yeah we model here it's {}", m.index_count),
    //     None => {}
    // }
    //return meshes;
}

fn log(str: String) {
    crate::log::log(format!("🎦model::{}", str));
}

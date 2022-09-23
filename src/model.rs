use bytemuck::{Pod, Zeroable};
use glam::{ivec3, vec4, IVec3, Mat4, Quat, Vec3, Vec4};
use itertools::izip;
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use rustc_hash::FxHashMap;
use std::{collections::HashMap, sync::Arc};
use wgpu::{util::DeviceExt, Device};

lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref CUBE: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
    pub static ref PLANE: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
    pub static ref CUSTOM: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
    pub static ref DICTIONARY:RwLock<HashMap<String,Arc<OnceCell<Model>> >> =RwLock::new(HashMap::new());
    /** map our */
    pub static ref INT_DICTIONARY:RwLock<HashMap<String,u32>> = RwLock::new(HashMap::new());
    pub static ref INT_MAP:RwLock<FxHashMap<u32,Arc<OnceCell<Model>> >> = RwLock::new(FxHashMap::default());
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
    pub fn rot90(&mut self) {
        //+x -> -y, +y -> +x
        let x = self._pos[0];
        self._pos[0] = -self._pos[1];
        self._pos[1] = x;
    }
    pub fn rot180(&mut self) {
        //+x-> -x, +y -> -y
        self._pos[0] = -self._pos[0];
        self._pos[1] = -self._pos[1];
    }
    pub fn rot270(&mut self) {
        //+x -> +y, +y-> -x
        let x = self._pos[0];
        self._pos[0] = self._pos[1];
        self._pos[1] = -x;
    }
    pub fn rotp90(&mut self) {
        let x = self._pos[0];
        self._pos[0] = 16 - self._pos[1];
        self._pos[1] = x;
    }
    pub fn rotp180(&mut self) {
        self._pos[0] = 16 - self._pos[0];
        self._pos[1] = 16 - self._pos[1];
    }
    pub fn rotp270(&mut self) {
        //+x -> +y, +y-> -x
        let x = self._pos[0];
        self._pos[0] = self._pos[1];
        self._pos[1] = 16 - x;
    }
    pub fn texture(&mut self, uv: Vec4) {
        self._tex[0] = (self._tex[0] * uv.z) + uv.x;
        self._tex[1] = (self._tex[1] * uv.w) + uv.y;
    }
    pub fn to_string(self) -> String {
        format!("({},{},{})", self._pos[0], self._pos[1], self._pos[2])
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
    /* OLD
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
    ];*/

    let vertex_data = [
        // top (0, 0, 1)
        vertex([0, 0, i], [0, 0, 1], [0., 1.]),
        vertex([i, 0, i], [0, 0, 1], [1., 1.]),
        vertex([i, i, i], [0, 0, 1], [1., 0.]),
        vertex([0, i, i], [0, 0, 1], [0., 0.]),
        // bottom (0, 0, -1)
        vertex([0, i, 0], [0, 0, -1], [0., 0.]),
        vertex([i, i, 0], [0, 0, -1], [1., 0.]),
        vertex([i, 0, 0], [0, 0, -1], [1., 1.]),
        vertex([0, 0, 0], [0, 0, -1], [0., 1.]),
        // right east (1, 0, 0)
        vertex([i, 0, 0], [1, 0, 0], [0., 1.]),
        vertex([i, i, 0], [1, 0, 0], [1., 1.]),
        vertex([i, i, i], [1, 0, 0], [1., 0.]),
        vertex([i, 0, i], [1, 0, 0], [0., 0.]),
        // left west (-1, 0, 0)
        vertex([0, 0, i], [-1, 0, 0], [0., 0.]),
        vertex([0, i, i], [-1, 0, 0], [1., 0.]),
        vertex([0, i, 0], [-1, 0, 0], [1., 1.]),
        vertex([0, 0, 0], [-1, 0, 0], [0., 1.]),
        // front south (0, 1, 0)
        vertex([i, i, 0], [0, 1, 0], [0., 1.]),
        vertex([0, i, 0], [0, 1, 0], [1., 1.]),
        vertex([0, i, i], [0, 1, 0], [1., 0.]),
        vertex([i, i, i], [0, 1, 0], [0., 0.]),
        // back north (0, -1, 0)
        vertex([i, 0, i], [0, -1, 0], [1., 0.]),
        vertex([0, 0, i], [0, -1, 0], [0., 0.]),
        vertex([0, 0, 0], [0, -1, 0], [0., 1.]),
        vertex([i, 0, 0], [0, -1, 0], [1., 1.]),
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
        name: str,
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

    let plane_model = Model {
        name: "plane".to_string(),
        vertex_buf: plane_vertex_buf,
        index_buf: plane_index_buf,
        index_format: wgpu::IndexFormat::Uint32,
        index_count: plane_index_data.len(),
        data: None,
    };
    PLANE.get_or_init(|| plane_model);

    let (cube_vertex_data, cube_index_data) = create_cube(16); // TODO 16
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
    let cube_model = Model {
        name: "cube".to_string(),
        vertex_buf: cube_vertex_buf,
        index_buf: cube_index_buf,
        index_format: wgpu::IndexFormat::Uint32,
        index_count: cube_index_data.len(),
        data: Some((cube_vertex_data, cube_index_data)),
    };
    CUBE.get_or_init(|| cube_model);
    DICTIONARY
        .write()
        .insert("cube".to_string(), Arc::clone(&CUBE));
}

pub fn cube_model() -> Arc<OnceCell<Model>> {
    Arc::clone(&CUBE)
}

pub fn plane_model() -> Arc<OnceCell<Model>> {
    Arc::clone(&PLANE)
}

pub fn get_model(str: &String) -> Arc<OnceCell<Model>> {
    if str == "plane" {
        return plane_model();
    }

    match DICTIONARY.read().get(str) {
        Some(model) => Arc::clone(model),
        None => cube_model(),
    }
    //Arc::clone(&custom)
}

/** Returns a model by verts that are able to be adjusted through translation of the actual verts */
pub fn get_adjustable_model(str: &String) -> Option<Arc<OnceCell<Model>>> {
    match DICTIONARY.read().get(str) {
        Some(model) => match model.get() {
            Some(_) => Some(Arc::clone(model)),
            _ => {
                log(format!("missing model data field for {}", str));
                None
            }
        },
        None => None,
    }
}

/** return model numerical index from a given model name */
pub fn get_model_index(str: &String) -> Option<u32> {
    match INT_DICTIONARY.read().get(str) {
        Some(u) => {
            // log(format!("🟢ye we got {} from {}", u, str));
            Some(u.clone())
        }
        None => None,
    }
}

pub fn search_model(str: &String) -> Vec<String> {
    let searcher = str.to_lowercase();
    let mut v = vec![];
    for i in DICTIONARY.read().keys() {
        if i.starts_with(&searcher) {
            v.push(i.clone());
        }
    }
    v
}

pub fn get_model_from_index(index: u32) -> Option<Arc<OnceCell<Model>>> {
    match INT_MAP.read().get(&index) {
        Some(model) => match model.get() {
            Some(_) => Some(Arc::clone(model)),
            _ => {
                // log(format!("missing model data field by index {}", index));
                None
            }
        },
        None => None,
    }
}
pub struct Model {
    pub name: String,
    pub vertex_buf: wgpu::Buffer,
    pub index_buf: wgpu::Buffer,
    pub index_format: wgpu::IndexFormat,
    pub index_count: usize,
    pub data: Option<(Vec<Vertex>, Vec<u32>)>,
}

/** entry model loader that is handed a buffer, probably from an unzip, then sends to central loader */
pub fn load_from_buffer(str: &String, slice: &Vec<u8>, device: &Device) {
    match gltf::import_slice(slice) {
        Ok((nodes, buffers, image_data)) => {
            load(str, nodes, buffers, image_data, device);
        }
        _ => {}
    }
}

/** entry model loader that first loads from disk and then sends to central load fn */
pub fn load_from_string(str: &String, device: &Device) {
    let target = str; //format!("assets/{}", str);

    match gltf::import(&target) {
        Ok((nodes, buffers, image_data)) => {
            load(str, nodes, buffers, image_data, device);
        }
        Err(err) => {
            log(format!("gltf err for {} -> {}", &target, err));
        }
    }
}

/** big honking central gltf/glb loader function that inserts the new model into our dictionary lookup */
fn load(
    str: &String,
    nodes: gltf::Document,
    buffers: Vec<gltf::buffer::Data>,
    image_data: Vec<gltf::image::Data>,
    device: &Device,
) {
    //let mut meshes: Vec<Mesh> = vec![];
    //let im1 = image_data.get(0).unwrap();
    // let bits = str.split(".").collect::<Vec<_>>();

    let p = std::path::PathBuf::from(&str);
    match p.file_stem() {
        Some(p) => {
            let name = p.to_os_string().into_string().unwrap(); //p.into_string().unwrap();

            let uv_adjust =
                crate::texture::load_tex_from_img(name.clone(), str.to_string(), &image_data);

            //let tex = Texture2D::from_rgba8(im1.width as u16, im1.height as u16, &im1.pixels);
            //tex.set_filter(FilterMode::Nearest);
            let mut first_instance = true;
            let mut meshes = vec![];

            // for node in nodes.nodes(){
            //     node.name()
            // }
            for mesh in nodes.meshes() {
                let mesh_name = match mesh.name() {
                    Some(n) => n,
                    _ => "" as &str,
                };

                first_instance = true;
                for primitive in mesh.primitives() {
                    // primitive.attributes().for_each(|(name, _)| {
                    //     name.
                    //     log(format!("attribute {:?}", name));
                    // });
                    let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

                    let verts_interleaved = izip!(
                        reader.read_positions().unwrap(),
                        //reader.read_normals().unwrap(),
                        //reader.read_colors(0).unwrap().into_rgb_f32().into_iter(),
                        reader.read_tex_coords(0).unwrap().into_f32(),
                        //reader.read_indices().unwrap()
                    );

                    let vertices = verts_interleaved
                        .map(|(pos, uv)| {
                            // position: Vec3::from(pos),
                            // uv: Vec2::from(uv),
                            // color: WHITE,
                            let mut vv = vertexx(pos, [0, 0, 0], uv);
                            //DEV are we always offseting the glb uv by the tilemap this way?
                            vv.texture(uv_adjust);
                            vv
                        })
                        .collect::<Vec<Vertex>>();

                    //         let verts = data
                    // .0
                    // .iter()
                    // .map(|v| {
                    //     let mut v2 = v.clone();
                    //     v2.trans(offset);
                    //     // v2.texture(uv);
                    //     v2
                    // })
                    // .collect::<Vec<Vertex>>();

                    if let Some(inds) = reader.read_indices() {
                        let indices = inds.into_u32().map(|u| u as u32).collect::<Vec<u32>>();

                        let mesh_vertex_buf =
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some(&format!("{}{}", str, " Mesh Vertex Buffer")),
                                contents: bytemuck::cast_slice(&vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });

                        let mesh_index_buf =
                            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some(&format!("{}{}", str, " Mesh Index Buffer")),
                                contents: bytemuck::cast_slice(&indices),
                                usage: wgpu::BufferUsages::INDEX,
                            });
                        println!("Load mesh {} with {} verts", name, vertices.len());
                        println!("ind #{} verts #{}", indices.len(), vertices.len());

                        let model = Model {
                            name: name.to_string(),
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
                            let model_cell = Arc::new(once);

                            //
                            log(format!(
                                "populated model {} with mesh named {}",
                                name, mesh_name
                            ));

                            meshes.push((mesh_name, model_cell));
                        }
                    };
                }
            }

            if meshes.len() > 0 {
                if meshes.len() == 1 {
                    let mesh = meshes.pop().unwrap();
                    let lower = name.to_lowercase();
                    log(format!("single model stored as {}", name));
                    index_model(lower.clone(), Arc::clone(&mesh.1));
                    DICTIONARY.write().insert(lower.to_string(), mesh.1);
                } else {
                    for m in meshes {
                        let compound_name = format!("{}.{}", name, m.0).to_lowercase();
                        log(format!("multi-model stored as {}", compound_name));
                        index_model(compound_name.clone(), Arc::clone(&m.1));
                        DICTIONARY.write().insert(compound_name, m.1);
                    }
                }
            }
        }

        None => {}
    }
}

/** Create a simple numerical key for our model and map it, returning that numerical key*/
fn index_model(key: String, model: Arc<OnceCell<Model>>) -> u32 {
    let mut guard = crate::texture::COUNTER.lock();
    let ind = *guard;
    println!(
        "indexed model {} with key {}",
        model.get().unwrap().name,
        ind
    );
    INT_MAP.write().insert(ind, model);

    INT_DICTIONARY.write().insert(key, ind);
    *guard += 1;
    ind
}

// TODO make this a cleaner function
pub fn edit_cube(name: String, textures: Vec<String>, device: &Device) {
    let m = CUBE.get().unwrap().clone();
    let (verts, inds) = (m).data.as_ref().unwrap().clone();
    // let (verts, inds) =  {
    //     Some(m) => {
    //         // println!("🟢we got a cube model");
    //         let data = cube.get().unwrap().data.as_ref().unwrap().clone();
    //         (data.0, data.1)
    //     }
    //     None => {
    //         //println!("🔴failed to locate cube model");
    //         crate::model::create_plane(16, None, None)
    //     }
    // };
    let mut uv = vec![];
    // println!("keys {}", crate::texture::list_keys());
    for t in textures {
        // println!("❓ {} ", t);
        uv.push(crate::texture::get_tex(&t));
    }
    // let t2 = textures.map(|t| t.clone());
    // let uvs = t2.map(|t| crate::texture::get_tex((&t.clone())));
    let verts2 = verts
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let mut v2 = v.clone();
            // v2.trans(offset.clone());

            v2.texture(uv[(i / 4 as usize)]);
            v2
        })
        .collect::<Vec<Vertex>>();

    let mesh_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{}{}", name, " Mesh Vertex Buffer")),
        contents: bytemuck::cast_slice(&verts2),
        usage: wgpu::BufferUsages::VERTEX,
    });

    let mesh_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(&format!("{}{}", name, " Mesh Index Buffer")),
        contents: bytemuck::cast_slice(&inds),
        usage: wgpu::BufferUsages::INDEX,
    });

    let model = Model {
        name: name.clone(),
        vertex_buf: mesh_vertex_buf,
        index_buf: mesh_index_buf,
        index_format: wgpu::IndexFormat::Uint32,
        index_count: inds.len(),
        data: Some((verts2, inds)),
    };

    let cell = OnceCell::new();
    cell.get_or_init(|| model);

    let arced_model = Arc::new(cell);
    let model_index = index_model(name.clone(), Arc::clone(&arced_model));
    DICTIONARY.write().insert(name.clone(), arced_model);
    log(format!(
        "created new model cube {} with index {}",
        name, model_index
    ));
}

pub fn reset() {
    INT_DICTIONARY.write().clear();
    INT_MAP.write().clear();
    let mut wr = DICTIONARY.write();
    wr.clear();
    wr.insert("cube".to_string(), Arc::clone(&CUBE));
}

fn log(str: String) {
    let m = format!("🎦model::{}", str);
    crate::log::log(m.clone());
    println!("{}", m);
}

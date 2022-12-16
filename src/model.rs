use bytemuck::{Pod, Zeroable};
use glam::{ivec3, vec4, IVec3, Mat4, Quat, Vec3, Vec4};
use itertools::izip;
use std::{collections::HashMap, rc::Rc};
use wgpu::{util::DeviceExt, Device};

use crate::{texture::TexManager, world::World};

pub struct ModelManager {
    pub CUBE: Rc<Model>,
    pub PLANE: Rc<Model>,
    // pub CUSTOM: Rc<Model>,
    pub DICTIONARY: HashMap<String, Rc<Model>>,
}

impl ModelManager {
    pub fn init(device: &Device) -> ModelManager {
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
        let PLANE = Rc::new(plane_model);

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
        let CUBE = Rc::new(cube_model);
        let mut DICTIONARY = HashMap::new();

        DICTIONARY.insert("cube".to_string(), CUBE.clone());
        ModelManager {
            CUBE,
            PLANE,
            // CUSTOM,
            DICTIONARY,
        }
    }

    pub fn cube_model(&self) -> Rc<Model> {
        Rc::clone(&self.CUBE)
    }

    pub fn plane_model(&self) -> Rc<Model> {
        Rc::clone(&self.PLANE)
    }

    pub fn get_model(&self, str: &String) -> Rc<Model> {
        if str == "plane" {
            return self.plane_model();
        }

        match self.DICTIONARY.get(str) {
            Some(model) => Rc::clone(model),
            None => self.cube_model(),
        }
        //Arc::clone(&custom)
    }
    pub fn get_model_or_not(&self, str: &String) -> Option<&Rc<Model>> {
        self.DICTIONARY.get(str)
    }

    /** Returns a model by verts that are able to be adjusted through translation of the actual verts */
    pub fn get_adjustable_model(&self, str: &String) -> Option<Rc<Model>> {
        match self.DICTIONARY.get(str) {
            Some(model) => Some(Rc::clone(model)),
            None => None,
        }
    }

    pub fn search_model(&self, str: &String, bundle: Option<u8>) -> Vec<String> {
        let searcher = str.to_lowercase();
        let mut v = vec![];
        for i in self.DICTIONARY.keys() {
            if i.starts_with(&searcher) {
                v.push(i.clone());
            }
        }
        v
    }

    /** big honking central gltf/glb loader function that inserts the new model into our dictionary lookup */
    fn load(
        &mut self,
        world: &mut World,
        bundle_id: u8,
        tex_manager: &mut TexManager,
        str: &String,
        nodes: gltf::Document,
        buffers: Vec<gltf::buffer::Data>,
        image_data: Vec<gltf::image::Data>,
        device: &Device,
        debug: bool,
    ) {
        let p = std::path::PathBuf::from(&str);
        match p.file_stem() {
            Some(p) => {
                let name = p.to_os_string().into_string().unwrap();

                let uv_arr =
                    tex_manager.load_tex_from_data(name.clone(), str.to_string(), &image_data);

                let uv_adjust = if uv_arr.len() == 0 {
                    vec![vec4(0., 0., 1., 1.)]
                } else {
                    uv_arr
                };

                //let tex = Texture2D::from_rgba8(im1.width as u16, im1.height as u16, &im1.pixels);
                //tex.set_filter(FilterMode::Nearest);
                let mut first_instance = true;
                let mut meshes = vec![];

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
                            reader.read_tex_coords(0).unwrap().into_f32(),
                        );

                        let vertices = verts_interleaved
                            .map(|(pos, uv)| {
                                // position: Vec3::from(pos),
                                // uv: Vec2::from(uv),
                                // color: WHITE,
                                let mut vv = vertexx(pos, [0, 0, 0], uv);
                                //DEV are we always offseting the glb uv by the tilemap this way?
                                vv.texture(uv_adjust[0]);
                                vv
                            })
                            .collect::<Vec<Vertex>>();

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

                            let compound_name = format!("{}.{}", name, mesh_name).to_lowercase();

                            let model = Model {
                                name: compound_name,
                                vertex_buf: mesh_vertex_buf,
                                index_buf: mesh_index_buf,
                                index_format: wgpu::IndexFormat::Uint32,
                                index_count: indices.len(),
                                data: Some((vertices, indices)),
                            };

                            // if first_instance {
                            //     first_instance = false;
                            let model_cell = Rc::new(model);

                            //
                            log(format!(
                                "populated model {} with mesh named {}",
                                name, mesh_name
                            ));

                            meshes.push((mesh_name, model_cell));
                            // }
                        };
                    }
                }

                if meshes.len() > 0 {
                    if meshes.len() == 1 {
                        let mesh = meshes.pop().unwrap();
                        let lower = name.to_lowercase();
                        log(format!("single model stored as {}", name));
                        //TODO is this mapped at some point?`
                        world.index_model(bundle_id, lower.clone(), Rc::clone(&mesh.1));
                        self.DICTIONARY
                            .insert(lower.to_string(), Rc::clone(&mesh.1));
                        let compound_name = format!("{}.{}", lower, mesh.0).to_lowercase();
                        log(format!("backup model stored as {}", compound_name));
                        self.DICTIONARY.insert(compound_name, mesh.1);
                    } else {
                        for m in meshes {
                            log(format!("multi-model stored as {}", m.1.name));
                            // m.1.name = compound_name;
                            //TODO is this mapped at some point?
                            world.index_model(bundle_id, m.1.name.clone(), Rc::clone(&m.1));
                            self.DICTIONARY.insert(m.1.name.clone(), m.1);
                        }
                    }
                }
            }

            None => {}
        }
    }

    /** entry model loader that is handed a buffer, probably from an unzip, then sends to central loader */
    pub fn load_from_buffer(
        &mut self,
        world: &mut World,
        bundle_id: u8,
        tex_manager: &mut TexManager,
        str: &String,
        slice: &Vec<u8>,
        device: &Device,
        debug: bool,
    ) {
        match gltf::import_slice(slice) {
            Ok((nodes, buffers, image_data)) => {
                self.load(
                    world,
                    bundle_id,
                    tex_manager,
                    str,
                    nodes,
                    buffers,
                    image_data,
                    device,
                    debug,
                );
            }
            _ => {}
        }
    }

    /** entry model loader that first loads from disk and then sends to central load fn */
    pub fn load_from_string(
        &mut self,
        world: &mut World,
        bundle_id: u8,
        tex_manager: &mut TexManager,
        str: &String,
        device: &Device,
        debug: bool,
    ) {
        let target = str; //format!("assets/{}", str);

        match gltf::import(&target) {
            Ok((nodes, buffers, image_data)) => {
                self.load(
                    world,
                    bundle_id,
                    tex_manager,
                    str,
                    nodes,
                    buffers,
                    image_data,
                    device,
                    debug,
                );
            }
            Err(err) => {
                log(format!("gltf err for {} -> {}", &target, err));
            }
        }
    }

    // TODO make this a cleaner function
    pub fn edit_cube(
        &mut self,
        world: &mut World,
        bundle_id: u8,
        tex_manager: &TexManager,
        name: String,
        textures: Vec<String>,
        device: &Device,
    ) {
        let m = self.CUBE.clone();
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
            uv.push(tex_manager.get_tex(&t));
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

        let rced_model = Rc::new(model);
        //TODO is this mapped at some point?
        let model_index = world.index_model(bundle_id, name.clone(), Rc::clone(&rced_model));
        self.DICTIONARY.insert(name.clone(), rced_model);
        log(format!("created new model cube {}", name));
    }

    pub fn reset(&mut self) {
        self.DICTIONARY.clear();
        self.DICTIONARY
            .insert("cube".to_string(), self.cube_model());
    }
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
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Sint16x4, 1 => Sint8x4, 2=> Float32x2];
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        // let vertex_attr = ;
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

pub struct Instance {
    position: Vec3,
    rotation: Quat,
}
impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: Mat4::from_rotation_translation(self.rotation, self.position).to_cols_array_2d(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    pub(crate) model: [[f32; 4]; 4],
}

impl InstanceRaw {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<InstanceRaw>() as wgpu::BufferAddress,
            // We need to switch from using a step mode of Vertex to Instance
            // This means that our shaders will only change to use the next
            // instance when the shader starts processing a new instance
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    // While our vertex shader only uses locations 0, and 1 now, in later tutorials we'll
                    // be using 2, 3, and 4, for Vertex. We'll start at slot 5 not conflict with them later
                    shader_location: 5,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
                // for each vec4. We'll have to reassemble the mat4 in
                // the shader.
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 6,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 7,
                    format: wgpu::VertexFormat::Float32x4,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
                    shader_location: 8,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
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

pub struct Model {
    pub name: String,
    pub vertex_buf: wgpu::Buffer,
    pub index_buf: wgpu::Buffer,
    pub index_format: wgpu::IndexFormat,
    pub index_count: usize,
    pub data: Option<(Vec<Vertex>, Vec<u32>)>,
}

fn log(str: String) {
    let m = format!("🎦model::{}", str);
    crate::log::log(m.clone());
    println!("{}", m);
}

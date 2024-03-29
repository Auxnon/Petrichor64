use bytemuck::{Pod, Zeroable};
use glam::{ivec3, vec4, IVec3, Mat4, Quat, Vec3, Vec4};
use itertools::izip;
use std::{collections::HashMap, rc::Rc};
#[cfg(feature = "headed")]
use wgpu::{util::DeviceExt, Device};

#[cfg(feature = "headed")]
use crate::texture::TexManager;
use crate::{
    log::{LogType, Loggy},
    world::World,
};

pub enum TextureStyle {
    Tri,
    Quad,
    TriRepeat,
    QuadRepeat,
}

pub struct ModelPacket {
    pub asset: String,
    pub textures: Vec<String>,
    pub vecs: Vec<[f32; 3]>,
    pub norms: Vec<[f32; 3]>,
    pub inds: Vec<u32>,
    pub uvs: Vec<[f32; 2]>,
    pub style: TextureStyle,
    /// Return channel to acknowledge success and unpause lua runtime
    pub sender: std::sync::mpsc::SyncSender<u8>,
}

pub struct ModelManager {
    pub CUBE: Rc<Model>,
    pub PLANE: Rc<Model>,
    pub DICTIONARY: HashMap<String, Rc<Model>>,
}

impl ModelManager {
    pub fn init(#[cfg(feature = "headed")] device: &Device) -> ModelManager {
        let (plane_vertex_data, plane_index_data) = create_plane(8, Some(ivec3(0, 0, 0)), None);
        #[cfg(feature = "headed")]
        let plane_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Plane Vertex Buffer"),
            contents: bytemuck::cast_slice(&plane_vertex_data),
            usage: wgpu::BufferUsages::VERTEX,
        });

        #[cfg(feature = "headed")]
        let plane_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Plane Index Buffer"),
            contents: bytemuck::cast_slice(&plane_index_data),
            usage: wgpu::BufferUsages::INDEX,
        });

        let plane_model = Model {
            base_name: "plane".to_string(),
            name: "plane".to_string(),
            index_count: plane_index_data.len(),
            data: None,
            #[cfg(feature = "headed")]
            vertex_buf: plane_vertex_buf,
            #[cfg(feature = "headed")]
            index_buf: plane_index_buf,
            #[cfg(feature = "headed")]
            index_format: wgpu::IndexFormat::Uint32,
        };
        let PLANE = Rc::new(plane_model);

        let (cube_vertex_data, cube_index_data) = create_cube(16); // TODO 16
        #[cfg(feature = "headed")]
        let cube_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cubes Vertex Buffer"),
            contents: bytemuck::cast_slice(&cube_vertex_data),
            usage: wgpu::BufferUsages::VERTEX,
        });
        #[cfg(feature = "headed")]
        let cube_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cubes Index Buffer"),
            contents: bytemuck::cast_slice(&cube_index_data),
            usage: wgpu::BufferUsages::INDEX,
        });
        let cube_model = Model {
            base_name: "cube".to_string(),
            name: "cube".to_string(),
            index_count: cube_index_data.len(),
            data: Some((cube_vertex_data, cube_index_data)),
            #[cfg(feature = "headed")]
            vertex_buf: cube_vertex_buf,
            #[cfg(feature = "headed")]
            index_buf: cube_index_buf,
            #[cfg(feature = "headed")]
            index_format: wgpu::IndexFormat::Uint32,
        };
        let CUBE = Rc::new(cube_model);
        let mut DICTIONARY = HashMap::new();

        DICTIONARY.insert("cube".to_string(), CUBE.clone());
        DICTIONARY.insert("plane".to_string(), PLANE.clone());
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

    pub fn get_model(&self, str: &str) -> Rc<Model> {
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
        #[cfg(feature = "headed")] device: &Device,
        #[cfg(feature = "headed")] tex_manager: &mut TexManager,
        world: &mut World,
        bundle_id: u8,
        str: &String,
        nodes: gltf::Document,
        buffers: Vec<gltf::buffer::Data>,
        image_data: Vec<gltf::image::Data>,

        loggy: &mut Loggy,
        debug: bool,
    ) {
        let p = std::path::PathBuf::from(&str);
        match p.file_stem() {
            Some(p) => {
                let base_name = p.to_os_string().into_string().unwrap().to_lowercase();

                #[cfg(not(feature = "headed"))]
                let uv_adjust = vec![vec4(0., 0., 1., 1.)];

                #[cfg(feature = "headed")]
                let uv_arr = tex_manager.load_tex_from_data(
                    base_name.clone(),
                    str.to_string(),
                    &image_data,
                    loggy,
                );

                #[cfg(feature = "headed")]
                let uv_adjust = if uv_arr.len() == 0 {
                    vec![vec4(0., 0., 1., 1.)]
                } else {
                    uv_arr
                };

                let mut first_instance = true;
                let mut meshes = vec![];

                for (mi, mesh) in nodes.meshes().enumerate() {
                    let mesh_name = match mesh.name() {
                        Some(name) => name.to_string(),
                        None => mi.to_string(),
                    }
                    .to_lowercase();

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
                                let mut vv = vertexx(pos, [0, 0, 0], uv);
                                //DEV are we always offseting the glb uv by the tilemap this way?
                                vv.texture(uv_adjust[0]);
                                vv
                            })
                            .collect::<Vec<Vertex>>();

                        if let Some(inds) = reader.read_indices() {
                            let indices = inds.into_u32().map(|u| u as u32).collect::<Vec<u32>>();

                            let model = Self::build_complex_model(
                                #[cfg(feature = "headed")]
                                device,
                                &base_name,
                                Some(&mesh_name),
                                vertices,
                                indices,
                            );

                            meshes.push(model);
                            // }
                        };
                    }
                }

                if meshes.len() > 0 {
                    if meshes.len() == 1 {
                        let mesh = meshes.pop().unwrap();
                        loggy.log(
                            LogType::Model,
                            &format!(
                                "single model stored as {} and {}",
                                mesh.base_name, mesh.name
                            ),
                        );

                        world.index_model(bundle_id, &mesh.base_name, Rc::clone(&mesh));
                        // world.index_texture_alias(bundle_id, name, direct)
                        self.DICTIONARY
                            .insert(mesh.base_name.clone(), Rc::clone(&mesh));
                        // let compound_name = format!("{}.{}", lower, mesh.0).to_lowercase();
                        // self.DICTIONARY.insert(compound_name, mesh.1);
                    } else {
                        for (i, m) in meshes.drain(..).enumerate() {
                            if i == 0 {
                                loggy.log(
                                    LogType::Model,
                                    &format!(
                                        "multi-model {} stored as base name {}",
                                        i, m.base_name
                                    ),
                                );
                                world.index_model(bundle_id, &m.base_name, Rc::clone(&m));
                                self.DICTIONARY.insert(m.base_name.clone(), Rc::clone(&m));
                            }
                            loggy.log(
                                LogType::Model,
                                &format!("multi-model {} stored as {}", i, m.name),
                            );
                            world.index_model(bundle_id, &m.name, Rc::clone(&m));
                            self.DICTIONARY.insert(m.name.clone(), m);
                        }
                    }
                }
            }

            None => {}
        }
    }

    fn build_complex_model(
        #[cfg(feature = "headed")] device: &Device,
        base_name: &str,
        mesh_name: Option<&str>,
        vertices: Vec<Vertex>,
        indices: Vec<u32>,
    ) -> Rc<Model> {
        let compound_name = match mesh_name {
            Some(n) => format!("{}.{}", base_name, n),
            None => base_name.to_owned(),
        };
        #[cfg(feature = "headed")]
        let mesh_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}{}", compound_name, " Mesh Vertex Buffer")),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        #[cfg(feature = "headed")]
        let mesh_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}{}", compound_name, " Mesh Index Buffer")),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        let model = Model {
            base_name: base_name.to_string(),
            name: compound_name,
            index_count: indices.len(),
            data: Some((vertices, indices)),
            #[cfg(feature = "headed")]
            vertex_buf: mesh_vertex_buf,
            #[cfg(feature = "headed")]
            index_buf: mesh_index_buf,
            #[cfg(feature = "headed")]
            index_format: wgpu::IndexFormat::Uint32,
        };
        let model_cell = Rc::new(model);

        model_cell
    }

    pub fn upsert_model(
        &mut self,
        #[cfg(feature = "headed")] device: &Device,
        #[cfg(feature = "headed")] tex_manager: &TexManager,
        world: &mut World,
        bundle_id: u8,
        name: &str,
        texture: Vec<String>,
        verts: Vec<[f32; 3]>,
        fnorms: Vec<[f32; 3]>,
        inds: Vec<u32>,
        uvs: Vec<[f32; 2]>,
        tex_style: TextureStyle,
        loggy: &mut Loggy,
        debug: bool,
    ) -> Option<String> {
        if debug {
            loggy.log(
                LogType::Model,
                &format!(
                    "Build inline model {} from {} verts, {} inds and texture {:?}",
                    name,
                    verts.len(),
                    inds.len(),
                    texture
                ),
            );
        }
        let t_count = texture.len();
        let uvs_match_v = uvs.len() == verts.len();
        let norms = if fnorms.len() != verts.len() {
            let mut norms = Vec::new();
            for _ in 0..verts.len() {
                norms.push([0, 0, 0]);
            }
            norms
        } else {
            fnorms
                .iter()
                .map(|n| {
                    [
                        (n[0] * 100.) as i8,
                        (n[1] * 100.) as i8,
                        (n[2] * 100.) as i8,
                    ]
                })
                .collect::<Vec<[i8; 3]>>()
        };
        let single_texture = t_count == 1;
        let vertices = if uvs_match_v || t_count > 0 {
            if t_count == 1 {
                #[cfg(feature = "headed")]
                let uv_adjust = tex_manager.get_tex(&texture[0]);
                if !uvs_match_v {
                    verts
                        .iter()
                        .enumerate()
                        .map(|(i, pos)| {
                            let mut vv = vertexx(*pos, norms[i], [0., 0.]);
                            // vv.texture(uv_adjust);
                            vv
                        })
                        .collect::<Vec<Vertex>>()
                } else {
                    verts
                        .iter()
                        .enumerate()
                        .map(|(i, pos)| {
                            let uv = uvs[i];
                            let n = norms[i];
                            let mut vv = vertexx(*pos, n, uv);
                            #[cfg(feature = "headed")]
                            vv.texture(uv_adjust);
                            vv
                        })
                        .collect::<Vec<Vertex>>()
                }
            } else {
                let step_size = match tex_style {
                    TextureStyle::Quad => 4,
                    _ => 3,
                };
                let mut tex_cycle = 0;
                #[cfg(feature = "headed")]
                let uv_adjusts: Vec<Vec4> =
                    texture.iter().map(|t| tex_manager.get_tex(t)).collect();
                #[cfg(feature = "headed")]
                let mut current_tex = uv_adjusts[0];
                verts
                    .iter()
                    .enumerate()
                    .map(|(i, pos)| {
                        let uv = uvs[i];
                        #[cfg(feature = "headed")]
                        if i % step_size == 0 && i > 0 {
                            tex_cycle += 1;
                            if tex_cycle >= t_count {
                                tex_cycle = 0;
                            }
                            current_tex = uv_adjusts[tex_cycle];
                        }
                        let mut vv = vertexx(*pos, norms[i], uv);
                        #[cfg(feature = "headed")]
                        vv.texture(current_tex);
                        vv
                    })
                    .collect::<Vec<Vertex>>()
            }
        } else {
            verts
                .iter()
                .enumerate()
                .map(|(i, pos)| vertexx(*pos, norms[i], [0., 0.]))
                .collect::<Vec<Vertex>>()
        };
        let indicies = if inds.len() == 0 {
            let mut inds2 = Vec::new();
            for i in 0..verts.len() {
                inds2.push(i as u32);
            }
            inds2
        } else {
            inds
        };
        let model = Self::build_complex_model(
            #[cfg(feature = "headed")]
            device,
            &name.to_lowercase(),
            None,
            vertices,
            indicies,
        );

        world.index_model(bundle_id, &model.base_name, Rc::clone(&model));

        match self.DICTIONARY.insert(model.base_name.clone(), model) {
            Some(m) => Some(m.base_name.clone()),
            _ => None,
        }
    }

    /** entry model loader that is handed a buffer, probably from an unzip, then sends to central loader */
    pub fn load_from_buffer(
        &mut self,
        #[cfg(feature = "headed")] device: &Device,
        #[cfg(feature = "headed")] tex_manager: &mut TexManager,
        world: &mut World,
        bundle_id: u8,
        str: &String,
        slice: &Vec<u8>,
        loggy: &mut Loggy,
        debug: bool,
    ) {
        match gltf::import_slice(slice) {
            Ok((nodes, buffers, image_data)) => {
                self.load(
                    #[cfg(feature = "headed")]
                    device,
                    #[cfg(feature = "headed")]
                    tex_manager,
                    world,
                    bundle_id,
                    str,
                    nodes,
                    buffers,
                    image_data,
                    loggy,
                    debug,
                );
            }
            _ => {}
        }
    }

    /** entry model loader that first loads from disk and then sends to central load fn */
    pub fn load_from_string(
        &mut self,
        #[cfg(feature = "headed")] device: &Device,
        #[cfg(feature = "headed")] tex_manager: &mut TexManager,
        world: &mut World,
        bundle_id: u8,
        str: &String,
        loggy: &mut Loggy,
        debug: bool,
    ) {
        let target = str; //format!("assets/{}", str);

        match gltf::import(&target) {
            Ok((nodes, buffers, image_data)) => {
                self.load(
                    #[cfg(feature = "headed")]
                    device,
                    #[cfg(feature = "headed")]
                    tex_manager,
                    world,
                    bundle_id,
                    str,
                    nodes,
                    buffers,
                    image_data,
                    loggy,
                    debug,
                );
            }
            Err(err) => {
                loggy.log(
                    LogType::ModelError,
                    &format!("gltf err for {} -> {}", &target, err),
                );
            }
        }
    }

    // TODO make this a cleaner function
    pub fn edit_cube(
        &mut self,
        #[cfg(feature = "headed")] device: &Device,
        #[cfg(feature = "headed")] tex_manager: &TexManager,
        world: &mut World,
        bundle_id: u8,
        name: String,
        textures: Vec<String>,
    ) {
        let m = self.CUBE.clone();
        let (verts, inds) = (m).data.as_ref().unwrap().clone();

        #[cfg(feature = "headed")]
        let mut uv = vec![];
        #[cfg(feature = "headed")]
        for t in textures {
            uv.push(tex_manager.get_tex(&t));
        }
        let verts2 = verts
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let mut v2 = v.clone();
                // v2.trans(offset.clone());
                #[cfg(feature = "headed")]
                v2.texture(uv[(i / 4 as usize)]);
                v2
            })
            .collect::<Vec<Vertex>>();

        #[cfg(feature = "headed")]
        let mesh_vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}{}", name, " Mesh Vertex Buffer")),
            contents: bytemuck::cast_slice(&verts2),
            usage: wgpu::BufferUsages::VERTEX,
        });

        #[cfg(feature = "headed")]
        let mesh_index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("{}{}", name, " Mesh Index Buffer")),
            contents: bytemuck::cast_slice(&inds),
            usage: wgpu::BufferUsages::INDEX,
        });

        let model = Model {
            base_name: name.clone(),
            name: name.clone(),
            index_count: inds.len(),
            data: Some((verts2, inds)),
            #[cfg(feature = "headed")]
            vertex_buf: mesh_vertex_buf,
            #[cfg(feature = "headed")]
            index_buf: mesh_index_buf,
            #[cfg(feature = "headed")]
            index_format: wgpu::IndexFormat::Uint32,
        };

        let rced_model = Rc::new(model);
        //TODO is this mapped at some point?
        world.index_model(bundle_id, &name, Rc::clone(&rced_model));
        self.DICTIONARY.insert(name.clone(), rced_model);
        // TODO log(format!("created new model cube {}", name));
    }

    pub fn reset(&mut self) {
        self.DICTIONARY.clear();
        self.DICTIONARY
            .insert("cube".to_string(), self.cube_model());
        self.DICTIONARY
            .insert("plane".to_string(), self.plane_model());
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
        let nx = self._normal[0];
        self._normal[0] = -self._normal[1];
        self._normal[1] = nx;
    }
    pub fn rotp180(&mut self) {
        self._pos[0] = 16 - self._pos[0];
        self._pos[1] = 16 - self._pos[1];
        self._normal[0] = -self._normal[0];
        self._normal[1] = -self._normal[1];
    }
    pub fn rotp270(&mut self) {
        //+x -> +y, +y-> -x
        let x = self._pos[0];
        self._pos[0] = self._pos[1];
        self._pos[1] = 16 - x;
        let nx = self._normal[0];
        self._normal[0] = self._normal[1];
        self._normal[1] = -nx;
    }
    pub fn texture(&mut self, uv: Vec4) {
        self._tex[0] = (self._tex[0] * uv.z) + uv.x;
        self._tex[1] = (self._tex[1] * uv.w) + uv.y;
    }
    pub fn to_string(self) -> String {
        format!("({},{},{})", self._pos[0], self._pos[1], self._pos[2])
    }

    #[cfg(feature = "headed")]
    const ATTRIBS: [wgpu::VertexAttribute; 3] =
        wgpu::vertex_attr_array![0 => Sint16x4, 1 => Sint8x4, 2=> Float32x2];

    #[cfg(feature = "headed")]
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        // let vertex_attr = ;
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

#[cfg(feature = "headed")]
pub struct Instance {
    position: Vec3,
    rotation: Quat,
}

#[cfg(feature = "headed")]
impl Instance {
    pub fn to_raw(&self) -> InstanceRaw {
        InstanceRaw {
            model: Mat4::from_rotation_translation(self.rotation, self.position).to_cols_array_2d(),
        }
    }
}

#[cfg(feature = "headed")]
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct InstanceRaw {
    pub(crate) model: [[f32; 4]; 4],
}

#[cfg(feature = "headed")]
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
            [o.x as i16 + size, o.y as i16 - size, o.z as i16],
            [0, 0, 1],
            [ex, ey],
        ),
        vertex(
            [o.x as i16 + size, o.y as i16 + size, o.z as i16],
            [0, 0, 1],
            [ex, sy],
        ),
        vertex(
            [o.x as i16 - size, o.y as i16 - size, o.z as i16],
            [0, 0, 1],
            [sx, ey],
        ),
        vertex(
            [o.x as i16 - size, o.y as i16 + size, o.z as i16],
            [0, 0, 1],
            [sx, sy],
        ),
    ];

    let index_data: &[u32] = &[0, 1, 2, 2, 1, 3];

    (vertex_data.to_vec(), index_data.to_vec())
}

fn build_model(
    #[cfg(feature = "headed")] device: &Device,
    base_name: &str,
    name: &str,
    verts: &Vec<Vertex>,
    inds: &Vec<u16>,
) -> Model {
    #[cfg(feature = "headed")]
    let vertex_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Chunk Vertex Buffer"),
        contents: bytemuck::cast_slice(verts),
        usage: wgpu::BufferUsages::VERTEX,
    });

    #[cfg(feature = "headed")]
    let index_buf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("Chunk Index Buffer"),
        contents: bytemuck::cast_slice(inds),
        usage: wgpu::BufferUsages::INDEX,
    });
    Model {
        base_name: base_name.to_string(),
        name: name.to_string(),
        index_count: inds.len(),
        data: None,
        #[cfg(feature = "headed")]
        vertex_buf,
        #[cfg(feature = "headed")]
        index_buf,
        #[cfg(feature = "headed")]
        index_format: wgpu::IndexFormat::Uint32,
    }
}

pub struct Model {
    /** base name of the model if it is a multi-part model in the format of: base_name  */
    pub base_name: String,
    /** direct name including sub mesh name in the format of: base_name.sub_name */
    pub name: String,
    pub index_count: usize,
    pub data: Option<(Vec<Vertex>, Vec<u32>)>,
    #[cfg(feature = "headed")]
    pub vertex_buf: wgpu::Buffer,
    #[cfg(feature = "headed")]
    pub index_buf: wgpu::Buffer,
    #[cfg(feature = "headed")]
    pub index_format: wgpu::IndexFormat,
}

// fn log(str: String) {
//     let m = format!("🎦model::{}", str);
//     crate::log::log(m.clone());
//     println!("{}", m);
// }

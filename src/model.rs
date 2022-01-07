use bytemuck::{Pod, Zeroable};
use gltf::{image::Data as ImageData, json::extensions::mesh, Texture};
use itertools::izip;
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use std::sync::Arc;
use wgpu::{util::DeviceExt, Device};

lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref cube: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
    pub static ref plane: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
    pub static ref custom: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    _pos: [i16; 4],
    _normal: [i8; 4],
    _tex: [f32; 2],
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

fn create_cube() -> (Vec<Vertex>, Vec<u16>) {
    let vertex_data = [
        // top (0, 0, 1)
        vertex([-1, -1, 1], [0, 0, 1], [0., 1.]),
        vertex([1, -1, 1], [0, 0, 1], [1., 1.]),
        vertex([1, 1, 1], [0, 0, 1], [1., 0.]),
        vertex([-1, 1, 1], [0, 0, 1], [0., 0.]),
        // bottom (0, 0, -1)
        vertex([-1, 1, -1], [0, 0, -1], [0., 0.]),
        vertex([1, 1, -1], [0, 0, -1], [1., 0.]),
        vertex([1, -1, -1], [0, 0, -1], [1., 1.]),
        vertex([-1, -1, -1], [0, 0, -1], [0., 1.]),
        // right (1, 0, 0)
        vertex([1, -1, -1], [1, 0, 0], [0., 0.]),
        vertex([1, 1, -1], [1, 0, 0], [1., 0.]),
        vertex([1, 1, 1], [1, 0, 0], [1., 1.]),
        vertex([1, -1, 1], [1, 0, 0], [0., 1.]),
        // left (-1, 0, 0)
        vertex([-1, -1, 1], [-1, 0, 0], [0., 0.]),
        vertex([-1, 1, 1], [-1, 0, 0], [1., 0.]),
        vertex([-1, 1, -1], [-1, 0, 0], [1., 1.]),
        vertex([-1, -1, -1], [-1, 0, 0], [0., 1.]),
        // front (0, 1, 0)
        vertex([1, 1, -1], [0, 1, 0], [0., 0.]),
        vertex([-1, 1, -1], [0, 1, 0], [1., 0.]),
        vertex([-1, 1, 1], [0, 1, 0], [1., 1.]),
        vertex([1, 1, 1], [0, 1, 0], [0., 1.]),
        // back (0, -1, 0)
        vertex([1, -1, 1], [0, -1, 0], [0., 0.]),
        vertex([-1, -1, 1], [0, -1, 0], [1., 0.]),
        vertex([-1, -1, -1], [0, -1, 0], [1., 1.]),
        vertex([1, -1, -1], [0, -1, 0], [0., 1.]),
    ];

    let index_data: &[u16] = &[
        0, 1, 2, 2, 3, 0, // top
        4, 5, 6, 6, 7, 4, // bottom
        8, 9, 10, 10, 11, 8, // right
        12, 13, 14, 14, 15, 12, // left
        16, 17, 18, 18, 19, 16, // front
        20, 21, 22, 22, 23, 20, // back
    ];

    (vertex_data.to_vec(), index_data.to_vec())
}

fn create_plane(size: i16) -> (Vec<Vertex>, Vec<u16>) {
    let vertex_data = [
        vertex([size, 0, 0], [0, 0, 1], [1., 1.]),
        vertex([size, size * 2, 0], [0, 0, 1], [1., 0.]),
        vertex([-size, 0, 0], [0, 0, 1], [0., 1.]),
        vertex([-size, size * 2, 0], [0, 0, 1], [0., 0.]),
    ];

    let index_data: &[u16] = &[0, 1, 2, 2, 1, 3];

    (vertex_data.to_vec(), index_data.to_vec())
}

pub fn init(device: &Device) {
    let (plane_vertex_data, plane_index_data) = create_plane(1);
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
        index_format: wgpu::IndexFormat::Uint16,
        index_count: plane_index_data.len(),
    };
    plane.get_or_init(|| planeModel);

    let (cube_vertex_data, cube_index_data) = create_cube();
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
        index_format: wgpu::IndexFormat::Uint16,
        index_count: cube_index_data.len(),
    };
    cube.get_or_init(|| cubeModel);

    load("package", device);
}
pub fn cube_model() -> Arc<OnceCell<Model>> {
    Arc::clone(&cube)
}
pub fn plane_model() -> Arc<OnceCell<Model>> {
    Arc::clone(&plane)
}

pub fn get_model(str: String) -> Arc<OnceCell<Model>> {
    if str == "plane" {
        return plane_model();
    }
    Arc::clone(&custom)
}
pub struct Model {
    pub vertex_buf: wgpu::Buffer,
    pub index_buf: wgpu::Buffer,
    pub index_format: wgpu::IndexFormat,
    pub index_count: usize,
}

pub fn load(str: &str, device: &Device) {
    let target = format!("assets/{}.glb", str);
    let (nodes, buffers, image_data) = gltf::import(target).unwrap();
    //let mut meshes: Vec<Mesh> = vec![];
    //let im1 = image_data.get(0).unwrap();

    crate::assets::load_tex_from_img("custom".to_string(), &image_data);

    //let tex = Texture2D::from_rgba8(im1.width as u16, im1.height as u16, &im1.pixels);
    //tex.set_filter(FilterMode::Nearest);
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
                let indices = inds.into_u32().map(|u| u as u16).collect::<Vec<u16>>();

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

                let customModel = Model {
                    vertex_buf: mesh_vertex_buf,
                    index_buf: mesh_index_buf,
                    index_format: wgpu::IndexFormat::Uint16,
                    index_count: indices.len(),
                };
                custom.get_or_init(|| customModel);

                //rand::srand(6);

                //mat.mul_vec4(other)
                //meshes.push(mesh);
            };
        }
    }
    match custom.get() {
        Some(m) => println!("yeah we model here it's {}", m.index_count),
        None => {}
    }
    //return meshes;
}

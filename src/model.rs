use bytemuck::{Pod, Zeroable};
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use std::{rc::Rc, sync::Arc};
use wgpu::{util::DeviceExt, Device};

lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref cube: Arc<OnceCell<Model>> = Arc::new(OnceCell::new());
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    _pos: [i8; 4],
    _normal: [i8; 4],
    _tex: [f32; 2],
}

fn vertex(pos: [i8; 3], nor: [i8; 3], tex: [f32; 2]) -> Vertex {
    Vertex {
        _pos: [pos[0], pos[1], pos[2], 1],
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

fn create_plane(size: i8) -> (Vec<Vertex>, Vec<u16>) {
    let vertex_data = [
        vertex([size, -size, 0], [0, 0, 1], [1., 0.]),
        vertex([size, size, 0], [0, 0, 1], [1., 1.]),
        vertex([-size, -size, 0], [0, 0, 1], [0., 0.]),
        vertex([-size, size, 0], [0, 0, 1], [0., 1.]),
    ];

    let index_data: &[u16] = &[0, 1, 2, 2, 1, 3];

    (vertex_data.to_vec(), index_data.to_vec())
}

pub fn init(device: &Device) {
    let (plane_vertex_data, plane_index_data) = create_plane(7);
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

    let (cube_vertex_data, cube_index_data) = create_cube();
    let cube_vertex_buf = Arc::new(
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cubes Vertex Buffer"),
            contents: bytemuck::cast_slice(&cube_vertex_data),
            usage: wgpu::BufferUsages::VERTEX,
        }),
    );
    let cube_index_buf = Arc::new(
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Cubes Index Buffer"),
            contents: bytemuck::cast_slice(&cube_index_data),
            usage: wgpu::BufferUsages::INDEX,
        }),
    );
    let cubeModel = Model {
        vertex_buf: cube_vertex_buf,
        index_buf: cube_index_buf,
        index_format: wgpu::IndexFormat::Uint16,
        index_count: cube_index_data.len(),
    };
    cube.get_or_init(|| cubeModel);
}
pub fn cube_model() -> Arc<OnceCell<Model>> {
    // Model
    Arc::clone(&cube)
}
pub struct Model {
    pub vertex_buf: Arc<wgpu::Buffer>,
    pub index_buf: Arc<wgpu::Buffer>,
    pub index_format: wgpu::IndexFormat,
    pub index_count: usize,
}

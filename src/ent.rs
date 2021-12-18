use std::rc::Rc;

use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EntityUniforms {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 4],
}

pub struct Ent {
    pub matrix: cgmath::Matrix4<f32>,
    pub rotation_speed: f32,
    pub color: wgpu::Color,
    pub vertex_buf: Rc<wgpu::Buffer>,
    pub index_buf: Rc<wgpu::Buffer>,
    pub index_format: wgpu::IndexFormat,
    pub index_count: usize,
    pub pos: cgmath::Vector3<f32>,
    pub uniform_offset: wgpu::DynamicOffset,
}

use crate::model::Model;
use bytemuck::{Pod, Zeroable};
use cgmath::{Decomposed, Deg, InnerSpace, Matrix, Quaternion, Rotation3, SquareMatrix, Vector3};
use once_cell::sync::OnceCell;
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EntityUniforms {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 4],
    pub uv_mod: [f32; 4],
    pub effects: [u32; 4],
}

pub struct Ent {
    pub matrix: cgmath::Matrix4<f32>,
    pub rotation: f32,
    pub color: wgpu::Color,
    pub pos: cgmath::Vector3<f32>,
    pub model: Arc<OnceCell<Model>>,
    pub uniform_offset: wgpu::DynamicOffset,
    pub tex: cgmath::Vector4<f32>,
    pub effects: cgmath::Vector4<u32>,
    pub brain: String,
}

impl Ent {
    pub fn new(
        offset: Vector3<f32>,
        angle: f32,
        scale: f32,
        rotation: f32,
        tex_name: String,
        model: String,
        uniform_offset: wgpu::DynamicOffset,
        billboarded: bool,
    ) -> Ent {
        let transform = Decomposed {
            disp: offset,
            rot: Quaternion::from_axis_angle(offset.normalize(), Deg(angle)),
            scale: scale,
        };

        Ent {
            matrix: cgmath::Matrix4::from(transform),
            rotation,
            color: wgpu::Color::GREEN,
            pos: offset,
            model: crate::model::get_model(model), //0.5, 1., 32. / 512., 32. / 512.
            //tex: cgmath::Vector4::new(0., 0., 0.5, 0.5), //crate::assets::get_tex(tex_name),
            // tex: cgmath::Vector4::new(0.5, 0., 32. / 512., 32. / 512.),
            tex: crate::texture::get_tex(tex_name), //cgmath::Vector4::new(0., 0., 1., 1.),
            uniform_offset,
            effects: cgmath::Vector4::new(if billboarded { 1 } else { 0 }, 0, 0, 0),
            brain: "".to_string(),
        }
    }
}

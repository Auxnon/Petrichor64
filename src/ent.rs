use crate::{
    lua_define::{self, LuaCore},
    lua_ent::LuaEnt,
    model::Model,
};
use bytemuck::{Pod, Zeroable};
use cgmath::{Decomposed, Deg, InnerSpace, Matrix, Quaternion, Rotation3, SquareMatrix, Vector3};
use mlua::Function;
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
    pub scale: f32,
    pub pos: cgmath::Vector3<f32>,
    pub vel: cgmath::Vector3<f32>,
    pub rot: cgmath::Vector3<f32>,
    pub model: Arc<OnceCell<Model>>,
    pub uniform_offset: wgpu::DynamicOffset,
    pub tex: cgmath::Vector4<f32>,
    pub effects: cgmath::Vector4<u32>,
    //pub brain: Option<Function<'lua>>,
    pub brain_name: Option<String>,
}

impl<'lua> Ent {
    pub fn new(
        offset: Vector3<f32>,
        angle: f32,
        scale: f32,
        rotation: f32,
        tex_name: String,
        model: String,
        uniform_offset: wgpu::DynamicOffset,
        billboarded: bool,
        brain: Option<String>,
    ) -> Ent {
        let transform = Decomposed {
            disp: offset,
            rot: Quaternion::from_axis_angle(offset.normalize(), Deg(angle)),
            scale: scale,
        };

        let mut brain_name = "".to_string();
        // let brain_func = match brain {
        //     Some(o) => {
        //         brain_name = o;
        //         let master = Arc::clone(&crate::lua_master);
        //         let core = master.lock();

        //         let a = core.get(o.clone());
        //         //let v = core.get(o.clone());
        //         Some(core)
        //     }
        //     None => None,
        // };

        Ent {
            matrix: cgmath::Matrix4::from(transform),
            rotation,
            rot: cgmath::Vector3::new(0., 0., 0.),
            color: wgpu::Color::GREEN,
            scale,
            pos: offset,
            vel: cgmath::Vector3::new(0., 0., 0.),
            model: crate::model::get_model(&model), //0.5, 1., 32. / 512., 32. / 512.
            //tex: cgmath::Vector4::new(0., 0., 0.5, 0.5), //crate::assets::get_tex(tex_name),
            // tex: cgmath::Vector4::new(0.5, 0., 32. / 512., 32. / 512.),
            tex: crate::texture::get_tex(tex_name), //cgmath::Vector4::new(0., 0., 1., 1.),
            uniform_offset,
            effects: cgmath::Vector4::new(if billboarded { 1 } else { 0 }, 0, 0, 0),
            //brain: None,
            brain_name: brain,
        }
    }
    pub fn to_lua(&self) -> LuaEnt {
        LuaEnt {
            x: self.pos.x,
            y: self.pos.y,
            z: self.pos.z,
            vel_x: self.vel.x,
            vel_y: self.vel.y,
            vel_z: self.vel.z,

            rot_x: self.rot.x,
            rot_y: self.rot.y,
            rot_z: self.rot.z,
        }
    }
    fn from_lua(&mut self, ent: LuaEnt) {
        self.pos.x = ent.x;
        self.pos.y = ent.y;
        self.pos.z = ent.z;

        self.vel.x = ent.vel_x;
        self.vel.y = ent.vel_y;
        self.vel.z = ent.vel_z;

        self.rot.x = ent.rot_x;
        self.rot.y = ent.rot_y;
        self.rot.z = ent.rot_z;
    }

    pub fn run(&mut self) {
        let lua_ent = self.to_lua();
        match &self.brain_name {
            Some(brain) => {
                let result = crate::lua_master
                    .lock()
                    .get()
                    .unwrap()
                    .call(brain.clone(), self.to_lua());
                self.from_lua(result);
            }
            None => {}
        }
    }
}

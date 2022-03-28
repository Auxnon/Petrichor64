use crate::{
    lua_define::{self, LuaCore},
    lua_ent::LuaEnt,
    model::Model,
};
use bytemuck::{Pod, Zeroable};

use glam::{IVec4, Mat4, Quat, UVec4, Vec3, Vec4};
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
    pub matrix: Mat4,
    pub rotation: f32,
    pub color: wgpu::Color,
    pub scale: f32,
    pub pos: Vec3,
    pub vel: Vec3,
    pub rot: Vec3,
    pub model: Arc<OnceCell<Model>>,
    pub uniform_offset: wgpu::DynamicOffset,
    pub tex: Vec4,
    /**hold the string name of texture for hot reloads*/
    tex_name: String,
    /**hold the string name of models for hot reloads*/
    model_name: String,
    pub effects: UVec4,
    //pub brain: Option<Function<'lua>>,
    pub brain_name: Option<String>,
}

impl<'lua> Ent {
    pub fn new(
        offset: Vec3,
        angle: f32,
        scale: f32,
        rotation: f32,
        tex_name: String,
        model: String,
        uniform_offset: wgpu::DynamicOffset,
        billboarded: bool,
        brain: Option<String>,
    ) -> Ent {
        //glam::Mat4::from_rotation_translation(rotation, translation);

        // let transform = Decomposed {
        //     disp: offset,
        //     rot: Quaternion::from_axis_angle(offset.normalize(), Deg(angle)),
        //     scale: scale,
        // };
        let quat = Quat::from_axis_angle(offset.normalize(), angle);

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
            matrix: Mat4::from_scale_rotation_translation(
                Vec3::new(scale, scale, scale),
                quat,
                offset,
            ),
            rotation,
            rot: Vec3::new(0., 0., 0.),
            color: wgpu::Color::GREEN,
            scale,
            pos: offset,
            vel: Vec3::new(0., 0., 0.),
            model: crate::model::get_model(&model), //0.5, 1., 32. / 512., 32. / 512.
            //tex: cgmath::Vector4::new(0., 0., 0.5, 0.5), //crate::assets::get_tex(tex_name),
            // tex: cgmath::Vector4::new(0.5, 0., 32. / 512., 32. / 512.),
            tex: crate::texture::get_tex(&tex_name), //cgmath::Vector4::new(0., 0., 1., 1.),
            tex_name,
            model_name: model,
            uniform_offset,
            effects: UVec4::new(if billboarded { 1 } else { 0 }, 0, 0, 0),
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

    pub fn hot_reload(&mut self) {
        self.tex = crate::texture::get_tex(&self.tex_name);
        println!("hot reload {}", self.tex);
        self.model = crate::model::get_model(&self.model_name)
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
        // println!("{:?}", &self.brain_name);

        match &self.brain_name {
            Some(brain) => {
                let result = crate::lua_master
                    .lock()
                    .get()
                    .unwrap()
                    .call(&brain, self.to_lua());
                self.from_lua(result);
            }
            None => {}
        }
    }
}

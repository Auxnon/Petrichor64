use crate::{lua_ent::LuaEnt, model::Model};
use bytemuck::{Pod, Zeroable};

use glam::{vec3, Mat4, Quat, UVec4, Vec3, Vec4};
use once_cell::sync::OnceCell;
use std::{ops::Mul, sync::Arc};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EntityUniforms {
    pub model: [[f32; 4]; 4],
    pub color: [f32; 4],
    pub uv_mod: [f32; 4],
    pub effects: [u32; 4],
}

#[derive(Clone)]
pub struct Ent {
    pub matrix: Mat4,
    pub rotation: f32,
    pub color: wgpu::Color,
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
    pub anim: Vec<Vec4>,
    pub anim_speed: u32,
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
            // rot: Vec3::new(0., 0., 0.),
            color: wgpu::Color::GREEN,
            // scale,
            // pos: offset,
            // vel: Vec3::new(0., 0., 0.),
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
            anim: vec![],
            anim_speed: 16,
        }
    }

    // pub fn to_lua(&self) -> LuaEnt {
    //     LuaEnt {
    //         x: self.pos.x,
    //         y: self.pos.y,
    //         z: self.pos.z,
    //         vel_x: self.vel.x,
    //         vel_y: self.vel.y,
    //         vel_z: self.vel.z,

    //         rot_x: self.rot.x,
    //         rot_y: self.rot.y,
    //         rot_z: self.rot.z,
    //         // id: -1.,
    //     }
    // }

    pub fn hot_reload(&mut self) {
        self.tex = crate::texture::get_tex(&self.tex_name); //crate::texture::get_tex(&self.tex_name);
        println!("hot reload {}", self.tex);
        self.model = crate::model::get_model(&self.model_name)
    }

    pub fn reparse(_lua: LuaEnt) {}

    pub fn build_meta(&self, lua: &LuaEnt) -> Mat4 {
        // let rotation = Mat4::from_rotation_z(self.rotation);

        let quat = Quat::from_axis_angle(vec3(0., 0., 1.), self.rotation);

        // let transform = cgmath::De composed {
        //     disp: entity.pos.mul(16.),
        //     rot: ),
        //     //rot: cgmath::Matrix4::from_angle_z(cgmath::Deg(entity.rotation)),
        //     scale: entity.scale * 16.,
        // };

        let s = 16.; // DEV entity.scale;
        let pos = vec3(lua.x as f32, lua.y as f32, lua.z as f32).mul(16.); // DEV entity.pos
        Mat4::from_scale_rotation_translation(vec3(s, s, s), quat, pos)
        // DEV i32
        /*
                let rotation = cgmath::Matrix4::from_angle_z(cgmath::Deg(entity.rotation));

                let v = entity.pos.mul(16.).cast::<i32>().unwrap();
                let rot = cgmath::Quaternion::<i32>::from_sv(
                    entity.rotation as i32,
                    cgmath::Vector3::<i32>::new(0, 0, 1),
                );
                let transform = cgmath::Decomposed::<cgmath::Vector3<i32>, cgmath::Quaternion<i32>> {
                    disp: v,
                    rot: rot,
                    //rot: cgmath::Matrix4::from_angle_z(cgmath::Deg(entity.rotation)),
        <<<<<<< Updated upstream
                    scale: (entity.scale * 16.) as i32,
        =======
                    scale: entity.scale,
        >>>>>>> Stashed changes
                };
                let matrix = cgmath::Matrix4::<i32>::from(transform);
                */
    }

    /**
     * provide iteration to determine how to animate if applicable
     */
    pub fn get_uniform(&self, lua: &LuaEnt, iteration: u64) -> EntityUniforms {
        let model = self.build_meta(lua);
        // self.matrix = model;
        let effects = [
            self.effects.x,
            self.effects.y,
            self.effects.z,
            self.effects.w,
        ];
        let uv_mod = if self.anim.len() > 0 {
            let a = self.anim[((iteration % (self.anim.len() as u32 * self.anim_speed) as u64)
                / self.anim_speed as u64) as usize];

            // println!("animating {} {}", self.anim.len(), a);
            [a.x, a.y, a.z, a.w]
        } else {
            [self.tex.x, self.tex.y, self.tex.z, self.tex.w]
        };
        let color = [
            self.color.r as f32,
            self.color.g as f32,
            self.color.b as f32,
            self.color.a as f32,
        ];
        EntityUniforms {
            model: model.to_cols_array_2d(),
            color,
            uv_mod,
            effects,
        }
    }
    // fn from_lua(&mut self, ent: LuaEnt) {
    //     self.pos.x = ent.x;
    //     self.pos.y = ent.y;
    //     self.pos.z = ent.z;

    //     self.vel.x = ent.vel_x;
    //     self.vel.y = ent.vel_y;
    //     self.vel.z = ent.vel_z;

    //     self.rot.x = ent.rot_x;
    //     self.rot.y = ent.rot_y;
    //     self.rot.z = ent.rot_z;
    // }

    pub fn run(&mut self) {
        // let lua_ent = self.to_lua();
        // println!("{:?}", &self.brain_name);

        // match &self.brain_name {
        //     Some(brain) => {
        //         let result = crate::lua_master
        //             .lock()
        //             .get()
        //             .unwrap()
        //             .call(&brain, self.to_lua());
        //         self.from_lua(result);
        //     }
        //     None => {}
        // }
    }
}

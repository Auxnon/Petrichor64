use crate::{
    lua_ent::LuaEnt,
    model::{Model, ModelManager},
    texture::{Anim, TexManager},
    Core,
};
use bytemuck::{Pod, Zeroable};

use glam::{vec3, vec4, Mat4, Quat, UVec4, Vec3, Vec4};
use std::{ops::Mul, rc::Rc};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EntityUniforms {
    pub uv_mod: [f32; 4],
    pub color: [f32; 4],
    pub effects: [f32; 4],
    pub model: [[f32; 4]; 4],
}

impl EntityUniforms {
    const ATTRIBS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        4=>Float32x4,5=>Float32x4,
    6=>Float32x4,
    7 => Float32x4, 8 => Float32x4, 9=> Float32x4,10=>Float32x4];
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        // let vertex_attr = ;
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<EntityUniforms>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &Self::ATTRIBS,
        }
    }
    // pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
    //     use std::mem;
    //     wgpu::VertexBufferLayout {
    //         array_stride: mem::size_of::<EntityUniforms>() as wgpu::BufferAddress,
    //         // We need to switch from using a step mode of Vertex to Instance
    //         // This means that our shaders will only change to use the next
    //         // instance when the shader starts processing a new instance
    //         step_mode: wgpu::VertexStepMode::Instance,
    //         attributes: &[
    //             wgpu::VertexAttribute {
    //                 offset: 0,
    //                 // While our vertex shader only uses locations 0, and 1 now, in later tutorials we'll
    //                 // be using 2, 3, and 4, for Vertex. We'll start at slot 5 not conflict with them later
    //                 shader_location: 5,
    //                 format: wgpu::VertexFormat::Float32x4,
    //             },
    //             // A mat4 takes up 4 vertex slots as it is technically 4 vec4s. We need to define a slot
    //             // for each vec4. We'll have to reassemble the mat4 in
    //             // the shader.
    //             wgpu::VertexAttribute {
    //                 offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
    //                 shader_location: 6,
    //                 format: wgpu::VertexFormat::Float32x4,
    //             },
    //             wgpu::VertexAttribute {
    //                 offset: mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
    //                 shader_location: 7,
    //                 format: wgpu::VertexFormat::Float32x4,
    //             },
    //             wgpu::VertexAttribute {
    //                 offset: mem::size_of::<[f32; 12]>() as wgpu::BufferAddress,
    //                 shader_location: 8,
    //                 format: wgpu::VertexFormat::Float32x4,
    //             },
    //         ],
    //     }
    // }
}

#[derive(Clone)]
pub struct Ent {
    pub matrix: Mat4,
    pub rotation: f32,
    pub color: wgpu::Color,
    pub model: Rc<Model>,
    pub uniform_offset: wgpu::DynamicOffset,
    pub tex: Vec4,
    /**hold the string name of texture for hot reloads*/
    tex_name: String,
    /**hold the string name of models for hot reloads*/
    model_name: String,
    pub effects: Vec4,
    // pub brain: Option<Function<'lua>>,
    // pub brain_name: Option<String>,
    anim: Option<Anim>,
    anim_it: u64,
    pub pos: Option<Vec3>,
}

impl<'lua> Ent {
    pub fn new_dynamic(
        tex_manager: &TexManager,
        model_manager: &ModelManager,
        offset: Vec3,
        angle: f32,
        scale: f32,
        rotation: f32,
        asset: String,
        uniform_offset: wgpu::DynamicOffset,
    ) -> Ent {
        // let (model_name,model,tex_name,tex,billboarded)=match tex_manager.get_tex_or_not(&asset.clone()){
        //     Some(t)=>{
        //         ("plane".to_string(),model_manager.PLANE.clone(),asset,t,true)
        //     }
        //     None=>{
        //         let m=model_manager.get_model(&asset);
        //         m.
        //         (asset.clone(),m,"".to_string(),vec4(0.,0.,0.,0.),false)
        //     }
        // };

        let (model_name, billboarded, model, tex) = match model_manager.get_model_or_not(&asset) {
            Some(m) => ("plane".to_string(), false, m, vec4(0., 0., 1., 1.)),
            None => (
                asset.clone(),
                true,
                &model_manager.PLANE,
                tex_manager.get_tex(&asset),
            ),
        };

        Ent::new_pure(
            offset,
            angle,
            scale,
            rotation,
            model_name,
            Rc::clone(model),
            asset.clone(),
            tex,
            uniform_offset,
            billboarded,
        )
    }

    pub fn new(
        core: &Core,
        offset: Vec3,
        angle: f32,
        scale: f32,
        rotation: f32,
        asset: String,
        tex_name: String,
        model_name: String,
        uniform_offset: wgpu::DynamicOffset,
        billboarded: bool,
    ) -> Ent {
        let model = core.model_manager.get_model(&model_name);
        let tex = core.tex_manager.get_tex(&tex_name);

        Ent::new_pure(
            offset,
            angle,
            scale,
            rotation,
            model_name,
            model,
            tex_name,
            tex,
            uniform_offset,
            billboarded,
        )
    }

    pub fn new_pure(
        offset: Vec3,
        angle: f32,
        scale: f32,
        rotation: f32,
        model_name: String,
        model: Rc<Model>,
        tex_name: String,
        tex: Vec4,
        uniform_offset: wgpu::DynamicOffset,
        billboarded: bool,
        // brain: Option<String>,
    ) -> Ent {
        // println!("make ent with tex {}",tex);
        let quat = Quat::from_axis_angle(offset.normalize(), angle);

        // let f=mlua::Lua::set_warning_function(&self, callback)
        Ent {
            matrix: Mat4::from_scale_rotation_translation(
                Vec3::new(scale, scale, scale),
                quat,
                offset,
            ),
            rotation,
            color: wgpu::Color::GREEN,
            model_name,
            model,
            tex,
            tex_name,
            uniform_offset,
            effects: Vec4::new(if billboarded { 1. } else { 0. }, 0., 0., 0.),
            anim: None,
            anim_it: 0,
            pos: None,
        }
    }

    pub fn hot_reload(&mut self, core: &Core) {
        self.tex = core.tex_manager.get_tex(&self.tex_name); //crate::texture::get_tex(&self.tex_name);
        println!("hot reload {}", self.tex);
        self.model = core.model_manager.get_model(&self.model_name)
    }

    pub fn reparse(_lua: LuaEnt) {}

    pub fn build_meta(&self, lua: &LuaEnt, parent: Option<&Mat4>) -> Mat4 {
        // Mat4::from_rotation_translation(rotation, translation);
        let offset = vec3(
            lua.offset[0] as f32,
            lua.offset[1] as f32,
            lua.offset[2] as f32,
        )
        .mul(16.);
        // println!("offset {:?}", offset);

        let quat = Quat::from_euler(
            glam::EulerRot::XYZ,
            lua.rot_x as f32,
            lua.rot_y as f32,
            lua.rot_z as f32,
        );
        // println!("build {} {:?}",self.model.name,parent);
        let s: f32 = (lua.scale as f32);
        // println!("scale {}",s);
        let pos = vec3(lua.x as f32, lua.y as f32, lua.z as f32).mul(16.); // DEV entity.pos
        let m = Mat4::from_translation(pos)
            * Mat4::from_scale(vec3(s, s, s))
            * Mat4::from_quat(quat)
            * Mat4::from_translation(offset);
        match parent {
            Some(p) => *p * m,
            None => m,
        }
    }

    /**
     * provide iteration to determine how to animate if applicable
     */
    pub fn get_uniform(
        &self,
        lua: &LuaEnt,
        iteration: u64,
        parent: Option<&Mat4>,
    ) -> EntityUniforms {
        let model = self.build_meta(lua, parent);
        self.get_uniforms_with_mat(lua, iteration, model)
    }

    /**
     * second half of get uniform, callable directly if matrix already available
     */
    pub fn get_uniforms_with_mat(
        &self,
        lua: &LuaEnt,
        iteration: u64,
        model: Mat4,
    ) -> EntityUniforms {
        let flipped = lua.flipped;
        // self.matrix = model;
        let effects = [
            self.effects.x * lua.scale as f32,
            lua.rot_z as f32,
            self.effects.z,
            self.effects.w,
        ];
        let uv_mod = match &self.anim {
            Some(anim) => {
                let diff = match iteration.checked_sub(self.anim_it) {
                    Some(u) => u,
                    _ => 0,
                };
                let a = if anim.once {
                    let iteratee = ((diff / anim.speed as u64) as usize).min(anim.frames.len() - 1);
                    anim.frames[iteratee]
                } else {
                    anim.frames[((diff % (anim.frames.len() as u32 * anim.speed) as u64)
                        / anim.speed as u64) as usize]
                };

                // println!("animating {} {}", self.anim.len(), a);
                // println!("animating {}",flipped);
                if flipped {
                    [a.x + a.z, a.y, -a.z, a.w]
                } else {
                    [a.x, a.y, a.z, a.w]
                }
            }
            None => {
                if flipped {
                    [self.tex.x + self.tex.z, self.tex.y, -self.tex.z, self.tex.w]
                } else {
                    [self.tex.x, self.tex.y, self.tex.z, self.tex.w]
                }
            }
        };

        let color = [
            self.color.r as f32,
            self.color.g as f32,
            self.color.b as f32,
            self.color.a as f32,
        ];
        EntityUniforms {
            color,
            uv_mod,
            effects,
            model: model.to_cols_array_2d(),
        }
    }

    /** specks? */
    pub fn simple_unfiforms(&self, iteration: u64) -> EntityUniforms {
        let mat = match self.pos {
            Some(p) => Mat4::from_translation(p).mul(16.),
            _ => self.matrix,
        };
        let effects = [
            self.effects.x,
            self.effects.y,
            self.effects.z,
            self.effects.w,
        ];
        let uv_mod = match &self.anim {
            Some(anim) => {
                let a = anim.frames[((iteration % (anim.frames.len() as u32 * anim.speed) as u64)
                    / anim.speed as u64) as usize];
                [a.x, a.y, a.z, a.w]
            }
            None => {
                let t = [self.tex.x, self.tex.y, self.tex.z, self.tex.w];
                t
            }
        };
        let color = [
            self.color.r as f32,
            self.color.g as f32,
            self.color.b as f32,
            self.color.a as f32,
        ];
        EntityUniforms {
            color,
            uv_mod,
            effects,
            model: mat.to_cols_array_2d(),
        }
    }

    pub fn set_anim(&mut self, a: Anim, iteration: u64) {
        self.anim = Some(a);
        self.anim_it = iteration;
    }
    pub fn remove_anim(&mut self) {
        if self.anim.is_some() {
            self.anim = None;
        }
    }
}

use crate::{lua_ent::LuaEnt, model::{Model, ModelManager}, Core, texture::TexManager};
use bytemuck::{Pod, Zeroable};

use glam::{vec3, Mat4, Quat, UVec4, Vec3, Vec4, vec4};
use std::{ops::Mul, rc::Rc};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EntityUniforms {
    pub uv_mod: [f32; 4],
    pub color: [f32; 4],
    pub effects: [u32; 4],
    pub model: [[f32; 4]; 4],
}

impl EntityUniforms {
    const ATTRIBS: [wgpu::VertexAttribute; 7] = wgpu::vertex_attr_array![
        4=>Float32x4,5=>Float32x4,
    6=>Uint32x4, 
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
    pub effects: UVec4,
    // pub brain: Option<Function<'lua>>,
    // pub brain_name: Option<String>,
    pub anim: Vec<Vec4>,
    pub anim_speed: u32,
    pub pos:Option<Vec3>
}

impl<'lua> Ent {
    pub fn new_dynamic(
        tex_manager:&TexManager,
        model_manager:&ModelManager,
        offset: Vec3,
        angle: f32,
        scale: f32,
        rotation: f32,
        asset: String,
        uniform_offset: wgpu::DynamicOffset,
    )->Ent{
        let (model_name,model,tex_name,tex,billboarded)=match tex_manager.get_tex_or_not(&asset.clone()){
            Some(t)=>{
                ("plane".to_string(),model_manager.PLANE.clone(),asset,t,true)
            }
            None=>{
                (asset.clone(),model_manager.get_model(&asset),"".to_string(),vec4(0.,0.,0.,0.),false)
            }
        };
        Ent::new_pure(offset,angle,scale,rotation,model_name,model,tex_name,tex,uniform_offset,billboarded)
    }

    pub fn new(
        core:&Core,
        offset: Vec3,
        angle: f32,
        scale: f32,
        rotation: f32,
        asset: String,
        tex_name: String,
        model_name: String,
        uniform_offset: wgpu::DynamicOffset,
        billboarded: bool)->Ent{
            let model=core.model_manager.get_model(&model_name);
            let tex=core.tex_manager.get_tex(&tex_name);
            
            Ent::new_pure(offset,angle,scale,rotation,model_name,model,tex_name,tex,uniform_offset,billboarded)
            
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
            effects: UVec4::new(if billboarded { 1 } else { 0 }, 0, 0, 0),
            anim: vec![],
            anim_speed: 16,
            pos:None
        }
    }

    pub fn hot_reload(&mut self,core: &Core) {
        self.tex = core.tex_manager.get_tex(&self.tex_name); //crate::texture::get_tex(&self.tex_name);
        println!("hot reload {}", self.tex);
        self.model = core.model_manager.get_model(&self.model_name)
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

        let s:f32 = 16.*(lua.scale as f32);
        // println!("scale {}",s);
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
            let t=
            [self.tex.x, self.tex.y, self.tex.z, self.tex.w];
            // println!("t {:?}",t);
            t
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

    pub fn simple_unfiforms(&self,iteration: u64) -> EntityUniforms{
        let mat=match self.pos{
            Some(p)=>{
                Mat4::from_translation(p).mul(16.)
            }
            _=>self.matrix
        };
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
            let t=
            [self.tex.x, self.tex.y, self.tex.z, self.tex.w];
            // println!("t {:?}",t);
            t
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
}

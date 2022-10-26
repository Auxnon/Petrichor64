#[derive(Clone)]
pub struct Speck {
    pub pos: Vec3,
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
    pub anim: Vec<Vec4>,
    pub anim_speed: u32,
}

impl Speck {
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
        let (model_name, model, tex_name, tex, billboarded) =
            match tex_manager.get_tex_or_not(&asset.clone()) {
                Some(t) => (
                    "plane".to_string(),
                    model_manager.PLANE.clone(),
                    asset,
                    t,
                    true,
                ),
                None => (
                    asset.clone(),
                    model_manager.get_model(&asset),
                    "".to_string(),
                    vec4(0., 0., 0., 0.),
                    false,
                ),
            };
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
}

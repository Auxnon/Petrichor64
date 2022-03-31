use glam::{vec4, Vec4};

/**  global state handler to go beyond threads */
pub struct SwitchBoard {
    pub background: Vec4,
    pub space: bool,
    pub h: f32,
}
impl SwitchBoard {
    pub fn new() -> SwitchBoard {
        SwitchBoard {
            background: vec4(0., 0., 0., 0.),
            space: false,
            h: 0f32,
        }
    }
}

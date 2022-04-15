use glam::{vec4, Vec4};

/**  global state handler to go beyond threads, such as acting as a shared variable pool between the main thread and the lua thread */
pub struct SwitchBoard {
    pub background: Vec4,
    pub space: bool,
    pub h: f32,
    pub tile_dirty: bool,
    pub tile_queue: Vec<Vec4>,
}
impl SwitchBoard {
    pub fn new() -> SwitchBoard {
        SwitchBoard {
            background: vec4(0., 0., 0., 0.),
            space: false,
            h: 0f32,
            tile_dirty: false,
            tile_queue: vec![],
        }
    }
}

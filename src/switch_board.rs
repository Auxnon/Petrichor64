use glam::{vec4, Vec4};

use crate::lua_ent::LuaEnt;

/**  global state handler to go beyond threads, such as acting as a shared variable pool between the main thread and the lua thread */
pub struct SwitchBoard {
    pub background: Vec4,
    pub space: bool,
    pub h: f32,
    pub dirty: bool,
    pub tile_queue: Vec<Vec4>,

    pub ent_queue: Vec<&'static LuaEnt>,
}
impl SwitchBoard {
    pub fn new() -> SwitchBoard {
        SwitchBoard {
            background: vec4(0., 0., 0., 0.),
            space: false,
            h: 0f32,
            dirty: false,
            tile_queue: vec![],
            ent_queue: vec![],
        }
    }
}

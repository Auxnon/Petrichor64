use std::collections::HashMap;

use glam::{vec2, vec3, Vec2, Vec3};

pub struct Global {
    pub space: bool,
    pub values: HashMap<String, f32>,
    pub mouse_click_pos: Vec2,
    pub mouse_active_pos: Vec2,
    pub mouse_delta: Vec2,
    pub camera_pos: Vec3,
}
impl Global {
    pub fn new() -> Global {
        Global {
            space: false,
            values: HashMap::new(),
            mouse_active_pos: vec2(0., 0.),
            mouse_click_pos: vec2(0., 0.),
            mouse_delta: vec2(0., 0.),
            camera_pos: vec3(0., 0., 0.),
        }
    }
    pub fn set(&mut self, key: String, v: f32) {
        self.values.insert(key, v);
    }
    pub fn get(&mut self, key: String) -> f32 {
        match self.values.get(&key) {
            Some(o) => *o,
            None => {
                self.values.insert(key, 0.);
                0.
            }
        }
    }
    // pub fn get_mut<'l>(&mut self, key: String) -> &'l f32 {
    //     match self.values.get_mut(&key) {
    //         Some(o) => o,
    //         None => {
    //             self.values.insert(key, 0.);
    //             self.values.get_mut(&key).unwrap()
    //         }
    //     }
    // }
}

use std::collections::{hash_map::Entry, HashMap};

use glam::{vec2, vec3, vec4, Vec2, Vec3, Vec4};
/** Global variable container intended for main thread only */
pub struct Global {
    pub values: HashMap<String, f32>,
    pub test: bool,
    pub mouse_click_pos: Vec2,
    pub mouse_active_pos: Vec2,
    pub mouse_delta: Vec2,
    pub console: bool,
    pub camera_pos: Vec3,
    pub background: Vec4,
    pub fps: f64,
    pub delayed: i32,
    /** The cursor unprojected pos in world space set by the render pipeline*/
    pub cursor_projected_pos: Vec3,
}
impl Global {
    pub fn new() -> Global {
        Global {
            values: HashMap::new(),
            console: false,
            mouse_active_pos: vec2(0., 0.),
            mouse_click_pos: vec2(0., 0.),
            mouse_delta: vec2(0., 0.),
            camera_pos: vec3(0., 0., 0.),
            cursor_projected_pos: vec3(0., 0., 0.),
            test: false,
            fps: 0.,
            background: vec4(0., 0., 0., 0.), //vec4(1., 0.2, 0.3, 1.0),
            delayed: 0,
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
    /** reference to the value so it can modified externally */
    pub fn get_mut(&mut self, key: String) -> &mut f32 {
        match self.values.entry(key) {
            Entry::Occupied(o) => o.into_mut(),
            Entry::Vacant(v) => v.insert(0.),
        }
        //map.entry(key).or_insert_with(|| default)
    }
}

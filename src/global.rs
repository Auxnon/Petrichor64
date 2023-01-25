use std::collections::{hash_map::Entry, HashMap};

use glam::{vec2, vec3, vec4, Vec2, Vec3, Vec4};

use crate::post::ScreenBinds;
/** Global variable container intended for main thread only */
pub struct Global {
    // pub values: HashMap<String, f32>,
    pub debug: bool,
    pub mouse_pos: Vec2,
    pub mouse_click_pos: Vec2,
    pub mouse_buttons: [f32; 4],
    pub fullscreen: bool,
    pub fullscreen_state: bool,
    /** whether to attempt to grab the mouse or not, does not indicate actual grabbed state or not */
    pub mouse_grab: bool,
    /** tracks whether the mouse is actively grabbed or not, the intent to grab can be different then the active grab, such as if a menu or console is open */
    pub mouse_grabbed_state: bool,
    pub simple_cam_rot: Vec2,
    pub smooth_cam_rot: Vec2,
    pub smooth_cam_pos: Vec3,
    pub mouse_delta: Vec2,
    pub scroll_delta: f32,
    pub game_controller: bool,
    pub console: bool,
    pub cam_pos: Vec3,
    pub debug_camera_pos: Vec3,
    pub background: Vec4,
    pub fps: f64,
    pub delayed: i32,
    pub iteration: u64,
    pub screen_effects: ScreenBinds,
    /** The cursor unprojected pos in world space set by the render pipeline*/
    pub cursor_projected_pos: Vec3,
    pub aliases: HashMap<String, String>,
    pub gui_params: GuiParams,
    pub state_changes: Vec<StateChange>,
    pub state_delay: u32,
    pub is_state_changed: bool,
    pub locked: bool,
    // pub loaded_directory: Option<String>,
}
impl Global {
    pub fn new() -> Global {
        Global {
            // values: HashMap::new(),
            console: true,
            game_controller: false,
            simple_cam_rot: vec2(std::f32::consts::FRAC_PI_2, 0.),
            smooth_cam_rot: vec2(std::f32::consts::FRAC_PI_2, 0.),
            mouse_pos: vec2(0., 0.),
            mouse_click_pos: vec2(0., 0.),
            mouse_buttons: [0.; 4],
            mouse_delta: vec2(0., 0.),
            cam_pos: vec3(0., 0., 0.),
            smooth_cam_pos: vec3(0., 0., 0.),
            debug_camera_pos: vec3(0., 0., 0.),
            cursor_projected_pos: vec3(0., 0., 0.),
            debug: false,
            fps: 0.,
            fullscreen: false,
            fullscreen_state: false,
            mouse_grab: false,
            mouse_grabbed_state: false,
            background: vec4(0., 0., 0., 0.), //vec4(1., 0.2, 0.3, 1.0),
            delayed: 0,
            iteration: 0,
            scroll_delta: 0.,
            screen_effects: ScreenBinds::new(),
            aliases: HashMap::new(),
            gui_params: GuiParams::new(),
            state_changes: Vec::new(),
            state_delay: 0,
            is_state_changed: false,
            locked: false,
            // loaded_directory: None,
        }
    }

    /** wipe only significant variables that could hurt a new game after reset */
    pub fn clean(&mut self) {
        self.mouse_pos.x = 0.;
        self.mouse_pos.y = 0.;
        self.mouse_grab = false;
        // self.fullscreen
        self.simple_cam_rot.x = std::f32::consts::FRAC_PI_2;
        self.simple_cam_rot.y = 0.;
        self.smooth_cam_rot.x = self.simple_cam_rot.x;
        self.smooth_cam_rot.y = self.simple_cam_rot.y;
        self.cam_pos.x = 0.;
        self.cam_pos.y = 0.;
        self.cam_pos.z = 0.;
        self.smooth_cam_pos.x = 0.;
        self.smooth_cam_pos.y = 0.;
        self.smooth_cam_pos.z = 0.;
        self.delayed = 0;
        self.iteration = 0;
        self.screen_effects = ScreenBinds::new();
    }

    // pub fn set(&mut self, key: String, v: f32) {
    //     self.values.insert(key, v);
    // }

    // pub fn get(&mut self, key: String) -> f32 {
    //     match self.values.get(&key) {
    //         Some(o) => *o,
    //         None => {
    //             self.values.insert(key, 0.);
    //             0.
    //         }
    //     }
    // }
    // /** reference to the value so it can modified externally */
    // pub fn get_mut(&mut self, key: String) -> &mut f32 {
    //     match self.values.entry(key) {
    //         Entry::Occupied(o) => o.into_mut(),
    //         Entry::Vacant(v) => v.insert(0.),
    //     }
    // }
}

pub enum StateChange {
    Resized,
    MouseGrabOn,
    MouseGrabOff,
    Quit,
}
pub enum GuiStyle {
    Aspect,
    Width,
    Height,
}
pub struct GuiParams {
    pub resolution: (u32, u32),
    pub style: GuiStyle,
    pub layout: (i8, i8),
    pub scaling: bool,
}
impl GuiParams {
    pub fn new() -> GuiParams {
        GuiParams {
            resolution: (320, 240),
            style: GuiStyle::Aspect,
            layout: (0, -1),
            scaling: false,
        }
    }
}

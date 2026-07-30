use std::collections::HashMap;

#[cfg(feature = "headed")]
use crate::post::ScreenBinds;
use glam::{vec2, vec3, vec4, Vec2, Vec3, Vec4};
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
    pub scroll_delta: (f32, f32),
    pub game_controller: bool,
    pub console: bool,
    pub cam_pos: Vec3,
    /// Directional "sun" for the 3D pass (L0 retro lighting). Defaults leave the
    /// scene fullbright (color 0 + ambient 1 => unchanged) so apps opt in via
    /// the `light` native.
    pub light_dir: Vec3,
    pub light_color: Vec3,
    pub light_ambient: f32,
    /// Hemisphere ambient (L2). amb_sky.w > 0 switches ambient from the flat
    /// `light_ambient` scalar to `mix(ground, sky, up)`: sky rgb from above,
    /// ground rgb from below, by surface normal.z.
    pub amb_sky: Vec4,
    pub amb_ground: Vec4,
    /// Distance fog (L2). xyz = fog rgb, w = far distance in world units where
    /// geometry is fully fogged. w = 0 disables it (default).
    pub fog_color: Vec4,
    pub debug_camera_pos: Vec3,
    pub background: Vec4,
    pub fps: f64,
    pub delayed: i32,
    pub iteration: u64,
    #[cfg(feature = "headed")]
    pub screen_effects: ScreenBinds,
    /** The cursor unprojected pos in world space set by the render pipeline*/
    pub cursor_projected_pos: Vec3,
    pub aliases: HashMap<String, String>,
    #[cfg(feature = "headed")]
    pub gui_params: GuiParams,
    pub state_changes: Vec<StateChange>,
    pub state_delay: u32,
    pub is_state_changed: bool,
    pub locked: bool,
    /** Whether we're in a boot or empty game state, showing the virtual console */
    pub boot_state: bool,
    /** Intial load or drag and drop intiated an animation sequence that upon completion will load this string if available */
    pub pending_load: Option<String>,
    // pub loaded_directory: Option<String>,
}
impl Global {
    pub fn new() -> Global {
        Global {
            // values: HashMap::new(),
            console: true,
            game_controller: false,
            simple_cam_rot: vec2(0., 0.),
            smooth_cam_rot: vec2(0., 0.),
            mouse_pos: vec2(0., 0.),
            mouse_click_pos: vec2(0., 0.),
            mouse_buttons: [0.; 4],
            mouse_delta: vec2(0., 0.),
            cam_pos: vec3(0., 0., 0.),
            // Fullbright by default: color 0 + ambient 1 => shade stays 1.
            light_dir: vec3(-0.3, -0.5, -0.8),
            light_color: vec3(0., 0., 0.),
            light_ambient: 1.,
            amb_sky: glam::vec4(0., 0., 0., 0.), // w=0 => flat ambient
            amb_ground: glam::vec4(0., 0., 0., 0.),
            fog_color: glam::vec4(0., 0., 0., 0.), // w=0 => fog off

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
            scroll_delta: (0., 0.),
            #[cfg(feature = "headed")]
            screen_effects: ScreenBinds::new(),
            aliases: HashMap::new(),
            #[cfg(feature = "headed")]
            gui_params: GuiParams::new(),
            state_changes: Vec::new(),
            state_delay: 0,
            is_state_changed: false,
            locked: false,
            boot_state: true,
            pending_load: None,
        }
    }

    /** wipe only significant variables that could hurt a new game after reset */
    pub fn clean(&mut self) {
        self.mouse_pos.x = 0.;
        self.mouse_pos.y = 0.;
        self.mouse_grab = false;
        // self.fullscreen
        self.simple_cam_rot.x = 0.;
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
        // Reset lighting/fog to defaults so state doesn't leak between apps
        // (fog off, fullbright): an app that never calls lamp/fog looks unlit.
        self.light_dir = vec3(-0.3, -0.5, -0.8);
        self.light_color = vec3(0., 0., 0.);
        self.light_ambient = 1.;
        self.amb_sky = glam::vec4(0., 0., 0., 0.);
        self.amb_ground = glam::vec4(0., 0., 0., 0.);
        self.fog_color = glam::vec4(0., 0., 0., 0.);
        #[cfg(feature = "headed")]
        {
            self.screen_effects = ScreenBinds::new();
        }
        // self.boot_state = false;
        self.pending_load = None;
        self.clean_app_attrs();
    }

    /// Reset the `attr` state an app *declares* for itself and must never inherit
    /// from whatever ran before it. Called on every app load as well as on reset —
    /// the same discipline as `SoundCommand::Reset` for the synth.
    ///
    /// The console lock is the one that bites: the boot fallback (`nil`) locks the
    /// console, so without this every game loaded afterwards inherited the lock and
    /// the console stayed unreachable. Note this only clears the flags; the load
    /// path re-enables the console widget itself (see `command.rs::load_app`).
    pub fn clean_app_attrs(&mut self) {
        self.locked = false;
        self.console = true;
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
    #[cfg(feature = "headed")]
    Resized,
    #[cfg(feature = "headed")]
    MouseGrabOn,
    #[cfg(feature = "headed")]
    MouseGrabOff,
    Quit,
    /** Runs initial config document parse and excutes and boot commands. Also checks against passed in command line args */
    Config,
    ModelChange(String),
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

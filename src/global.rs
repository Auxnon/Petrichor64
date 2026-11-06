use std::collections::HashMap;

/// `shdw()` defaults: map edge length (texels) and sun frustum half-width (tiles).
pub const SHADOW_SIZE_DEFAULT: u32 = 128;
pub const SHADOW_RANGE_DEFAULT: f32 = 25.0;

/// One finger or pen on the screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touching {
    /// winit's touch id
    pub id: u64,
    /// normalised 0..1, like the mouse
    pub x: f32,
    pub y: f32,
    /// a stylus rather than a finger
    pub pen: bool,
    /// 0..1; a screen that can't tell reports 1 (or 0)
    pub pressure: f32,
}

/// How many fingers `touches()` reports at once. Phones track ten, but past a
/// few no gesture needs them, and each slot costs three floats a frame.
pub const TOUCH_SLOTS: usize = 5;
/// Floats per touch slot in `ControlState` (see `pack_touches`).
pub const TOUCH_FLOATS: usize = 4;

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
    /// Every finger on the screen, for `touches()`: slot i holds the finger
    /// (or pen). A finger keeps its slot while it's down, so a script can follow
    /// one across frames by slot.
    pub touches: [Option<Touching>; TOUCH_SLOTS],
    /// The edges the system draws over (Android's status and navigation bars),
    /// in window pixels: top, right, bottom, left. Zero where nothing covers
    /// the window, as on desktop. `attr().inset`.
    pub safe_inset: [f32; 4],
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
    /// Orthographic half-height of the view, in world units; 0 is perspective.
    pub cam_ortho: f32,
    /// Directional "sun" for the 3D pass (L0 retro lighting). Defaults leave the
    /// scene fullbright (color 0 + ambient 1 => unchanged) so apps opt in via
    /// the `lum` native.
    pub light_dir: Vec3,
    pub light_color: Vec3,
    pub light_ambient: f32,
    /// Hemisphere ambient (L2). amb_sky.w > 0 switches ambient from the flat
    /// `light_ambient` scalar to `mix(ground, sky, up)`: sky rgb from above,
    /// ground rgb from below, by surface normal.z. Sun-only — a cone/sphere
    /// light has a real position, so "above/below" doesn't apply to it.
    pub amb_sky: Vec4,
    pub amb_ground: Vec4,
    /// Which shape `lum{}` currently is: 0=sun (today's directional
    /// behavior, default), 1=cone (spot), 2=sphere (point). See guide/lum.md.
    pub light_shape: u8,
    /// Tile-space position for a cone/sphere light (unused for sun) — same
    /// raw-until-render convention as `cam_pos` (scaled ×16 to world space at
    /// the point of use, in `render.rs`, not stored here).
    pub light_pos: Vec3,
    /// Falloff distance for a cone/sphere light, tile units (unused for sun).
    pub light_range: f32,
    /// Half-angle in radians for a cone light (unused for sun/sphere).
    pub light_angle: f32,
    /// Whether the shadow pass (from the current `lum` light) is enabled —
    /// see `shdw()`/`guide/shdw.md`. Off by default: zero behavior change for
    /// apps that never call it, same discipline as the chip/mon work.
    pub shadow_on: bool,
    /// Shadow map edge length in texels, as asked for by `shdw{size=}`; the
    /// renderer clamps it to what the device supports.
    pub shadow_size: u32,
    /// Half-width of the sun's shadow frustum in tiles (`shdw{range=}`): the
    /// area around the camera that can receive shadows. Smaller = sharper.
    pub shadow_range: f32,
    /// Whether shading is Gouraud (per-vertex, interpolated) instead of the
    /// default per-fragment "smooth" path — see `gour()`/`guide/gour.md`. Off
    /// by default, matching PLAN.md's original suggested default being
    /// deliberately not flipped this session.
    pub gouraud: bool,
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
    /// Graphics "chip" preset: 0=R00 (modern), 1=R43 (N64-ish), 2=R30 (PS1-ish).
    /// Drives `screen_effects.crt_resolution`/`vertex_snap`/`fog` and the main
    /// texture sampler's filter mode — see `MainCommmand::SetChip`. Plain state,
    /// not headed-gated (harmless to track headlessly), unlike `screen_effects`
    /// itself which only exists in a headed build.
    pub chip: u8,
    /// Display "monitor" preset: 0=LCD, 1=Slot (slot-mask CRT), 2=Grille
    /// (aperture-grille CRT). Drives the rest of `screen_effects` (the CRT
    /// post-pass knobs) — see `MainCommmand::SetMonitor`.
    pub monitor: u8,
    /** The cursor unprojected pos in world space set by the render pipeline*/
    pub cursor_projected_pos: Vec3,
    /// Last frame's view/projection, kept so the cursor ray can be re-traced at the
    /// *start* of a frame — before Lua reads it — rather than only during render,
    /// which left `cursor_projected_pos` a frame behind the pointer. See
    /// `Core::refresh_cursor_ray`. `None` until the first frame is rendered.
    pub last_cam_matrices: Option<(glam::Mat4, glam::Mat4)>,
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
            touches: [None; TOUCH_SLOTS],
            safe_inset: [0.; 4],
            mouse_delta: vec2(0., 0.),
            cam_pos: vec3(0., 0., 0.),
            cam_ortho: 0.,
            // Fullbright by default: color 0 + ambient 1 => shade stays 1.
            light_dir: vec3(-0.3, -0.5, -0.8),
            light_color: vec3(0., 0., 0.),
            light_ambient: 1.,
            amb_sky: glam::vec4(0., 0., 0., 0.), // w=0 => flat ambient
            amb_ground: glam::vec4(0., 0., 0., 0.),
            fog_color: glam::vec4(0., 0., 0., 0.), // w=0 => fog off
            light_shape: 0,
            light_pos: vec3(0., 0., 0.),
            light_range: 0.,
            light_angle: 0.,
            shadow_on: false,
            shadow_size: SHADOW_SIZE_DEFAULT,
            shadow_range: SHADOW_RANGE_DEFAULT,
            gouraud: false,

            smooth_cam_pos: vec3(0., 0., 0.),
            debug_camera_pos: vec3(0., 0., 0.),
            cursor_projected_pos: vec3(0., 0., 0.),
            last_cam_matrices: None,
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
            chip: 0,
            monitor: 0,
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

    /// A finger landed or moved (`down`), or lifted. A new finger takes the
    /// first free slot; one past `TOUCH_SLOTS` isn't reported.
    pub fn track_touch(&mut self, t: Touching, down: bool) {
        let at = self.touches.iter().position(|s| matches!(s, Some(s) if s.id == t.id));
        match (at, down) {
            (Some(i), true) => self.touches[i] = Some(t),
            (Some(i), false) => self.touches[i] = None,
            (None, true) => {
                if let Some(i) = self.touches.iter().position(|s| s.is_none()) {
                    self.touches[i] = Some(t);
                }
            }
            (None, false) => {}
        }
    }

    /// Write the touch slots into a `ControlState` analog array, from
    /// `ANALOG_TOUCH` on: per slot, `TOUCH_FLOATS` of them: what's in it (0
    /// nothing, 1 a finger, 2 a pen), its x and y, and its pressure.
    pub fn pack_touches(&self, analog: &mut [f32]) {
        for (i, t) in self.touches.iter().enumerate() {
            let at = crate::types::ANALOG_TOUCH + TOUCH_FLOATS * i;
            let v = match t {
                Some(t) => [if t.pen { 2.0 } else { 1.0 }, t.x, t.y, t.pressure],
                None => [0.0; TOUCH_FLOATS],
            };
            analog[at..at + TOUCH_FLOATS].copy_from_slice(&v);
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
        self.cam_ortho = 0.;
        self.smooth_cam_pos.x = 0.;
        self.smooth_cam_pos.y = 0.;
        self.smooth_cam_pos.z = 0.;
        self.delayed = 0;
        self.iteration = 0;
        self.reset_lighting_and_fog();
        #[cfg(feature = "headed")]
        {
            self.screen_effects = ScreenBinds::new();
        }
        // self.boot_state = false;
        self.pending_load = None;
        self.clean_app_attrs();
    }

    /// Reset lighting/fog (`lum`/`fog`) to defaults so state doesn't leak
    /// between apps (fog off, fullbright): an app that never calls lum/fog
    /// must look unlit, not inherit whatever the previous one left behind.
    ///
    /// Shared by `clean()` (hard/soft reset) and `clean_app_attrs()` (every
    /// app load) rather than only living in one of them — `fog_color` used to
    /// reset solely via `clean()`, which not every load path calls first, so
    /// distance fog set by `fog()` could survive into the next app exactly
    /// the way `attr{fog=...}` did before `clean_app_attrs` existed.
    pub fn reset_lighting_and_fog(&mut self) {
        self.light_dir = vec3(-0.3, -0.5, -0.8);
        self.light_color = vec3(0., 0., 0.);
        self.light_ambient = 1.;
        self.amb_sky = glam::vec4(0., 0., 0., 0.);
        self.amb_ground = glam::vec4(0., 0., 0., 0.);
        self.fog_color = glam::vec4(0., 0., 0., 0.);
        self.light_shape = 0;
        self.light_pos = vec3(0., 0., 0.);
        self.light_range = 0.;
        self.light_angle = 0.;
        self.shadow_on = false;
        self.shadow_size = SHADOW_SIZE_DEFAULT;
        self.shadow_range = SHADOW_RANGE_DEFAULT;
        self.gouraud = false;
    }

    /// Reset the `attr` state an app *declares* for itself and must never inherit
    /// from whatever ran before it. Called on every app load as well as on reset —
    /// the same discipline as `SoundCommand::Reset` for the synth.
    ///
    /// The console lock is the one that bites: the boot fallback (`nil`) locks the
    /// console, so without this every game loaded afterwards inherited the lock and
    /// the console stayed unreachable.
    ///
    /// Deliberately does NOT touch `console`. That flag is whether the console is
    /// currently *open*, and an open console swallows all keyboard input (see
    /// `controls.rs`) — forcing it true here left games unplayable, keys and all.
    /// Clearing `locked` is enough: backtick can toggle the console again.
    /// Reset the `attr` state an app can set, so none of it leaks into the next one.
    ///
    /// This only cleared the console lock, which meant every *screen effect* survived
    /// an app load. The boot logo sets `attr{fog=200}` on its first line, so every app
    /// loaded after it inherited a fog deep enough to swallow the scene — in an app
    /// that never mentions fog. Curvature, bleed, glitch and the rest leaked the same
    /// way; fog was simply the one you could not miss.
    ///
    /// `ScreenBinds::new()` is the same default a cold boot starts from, so this is
    /// "as if nothing had been loaded yet" rather than a second list of values to keep
    /// in step.
    pub fn clean_app_attrs(&mut self) {
        self.locked = false;
        self.chip = 0;
        self.monitor = 0;
        // Screen effects only exist in a headed build.
        #[cfg(feature = "headed")]
        {
            self.screen_effects = ScreenBinds::new();
        }
        // Distance fog/lighting are set by Lua (`fog`/`lum`), not `attr`, but
        // leak the exact same way if left out of a per-load reset — see
        // `reset_lighting_and_fog`.
        self.reset_lighting_and_fog();
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
    /// `attr{gui_px=}`: one gui pixel is this many logical (CSS, dp) pixels,
    /// so a big window gets a finer gui; never coarser than `resolution`.
    /// 0 is off: the gui is `resolution` at any window size.
    pub px: f32,
}
impl GuiParams {
    pub fn new() -> GuiParams {
        GuiParams {
            resolution: (320, 240),
            style: GuiStyle::Aspect,
            layout: (0, -1),
            scaling: false,
            px: 0.,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Loading an app must not inherit the previous app's `attr` state. The boot logo
    /// sets `attr{fog=200}` on its first line, and only the console lock was being
    /// cleared, so every app after it ran with a fog deep enough to hide the scene —
    /// including apps that never mention fog.
    /// `shdw{size=, range=}` is per-app state like the rest of the lighting:
    /// a game that sharpens its shadows must not hand that to the next app.
    #[test]
    fn shadow_tuning_resets_on_load() {
        let mut g = Global::new();
        g.shadow_on = true;
        g.shadow_size = 2048;
        g.shadow_range = 6.;

        g.clean_app_attrs();

        assert!(!g.shadow_on);
        assert_eq!(g.shadow_size, SHADOW_SIZE_DEFAULT);
        assert_eq!(g.shadow_range, SHADOW_RANGE_DEFAULT);
    }

    #[test]
    #[cfg(feature = "headed")]
    fn app_attrs_do_not_leak_into_the_next_app() {
        let fresh = ScreenBinds::new();
        let mut g = Global::new();

        // Stand in for what the logo (or any app) leaves behind.
        g.screen_effects.fog = 200.;
        g.screen_effects.crt_resolution = 999.;
        g.chip = 2; // R30
        g.monitor = 2; // Grille
        g.locked = true;

        g.clean_app_attrs();

        assert_eq!(g.screen_effects.fog, fresh.fog, "fog must not carry over");
        assert_eq!(
            g.screen_effects.crt_resolution, fresh.crt_resolution,
            "nor any other screen effect"
        );
        assert_eq!(g.chip, 0, "chip must reset to R00");
        assert_eq!(g.monitor, 0, "monitor must reset to LCD");
        assert!(!g.locked, "and the console lock still clears");
    }

    /// Distance fog (the `fog()` native's `fog_color`, not `attr`'s screen-effect
    /// `fog` scalar covered above) used to reset only in `clean()`, which not every
    /// app-load path calls — `clean_app_attrs()`, called on *every* load, left it
    /// untouched, so an app that called `fog()` could leave the next app fogged
    /// even though that next app never mentions fog.
    #[test]
    fn distance_fog_does_not_leak_into_the_next_app() {
        let mut g = Global::new();

        g.fog_color = glam::vec4(0.4, 0.4, 0.6, 40.0);
        g.light_color = vec3(1., 1., 1.);
        g.light_ambient = 0.1;

        g.clean_app_attrs();

        assert_eq!(
            g.fog_color,
            glam::vec4(0., 0., 0., 0.),
            "fog_color must reset to off on every app load, not just a hard/soft reset"
        );
        assert_eq!(g.light_color, vec3(0., 0., 0.));
        assert_eq!(g.light_ambient, 1.);
    }
}

#[cfg(test)]
mod touch_tests {
    use super::*;

    fn at(id: u64, x: f32, y: f32) -> Touching {
        Touching { id, x, y, pen: false, pressure: 1.0 }
    }

    fn ids(g: &Global) -> Vec<Option<u64>> {
        g.touches.iter().map(|t| t.map(|t| t.id)).collect()
    }

    #[test]
    fn a_finger_keeps_its_slot_and_a_lifted_slot_is_reused() {
        let mut g = Global::new();
        g.track_touch(at(7, 0.1, 0.2), true);
        g.track_touch(at(9, 0.5, 0.5), true);
        g.track_touch(at(7, 0.3, 0.4), true); // moved
        assert_eq!(g.touches[0], Some(at(7, 0.3, 0.4)));
        g.track_touch(at(7, 0.3, 0.4), false); // the first lifts: the second stays put
        assert_eq!(ids(&g)[..2], [None, Some(9)]);
        g.track_touch(at(11, 0.9, 0.9), true);
        assert_eq!(ids(&g)[..2], [Some(11), Some(9)]);
        g.track_touch(at(42, 0., 0.), false); // a lift never seen landing
        assert_eq!(ids(&g)[..2], [Some(11), Some(9)]);
    }

    #[test]
    fn fingers_past_the_slots_are_dropped_and_packing_marks_pens_and_empty_slots() {
        let mut g = Global::new();
        for id in 0..(TOUCH_SLOTS as u64 + 2) {
            g.track_touch(at(id, 0.5, 0.25), true);
        }
        assert!(g.touches.iter().all(|t| t.is_some()));
        g.track_touch(at(1, 0., 0.), false);
        g.track_touch(Touching { id: 2, x: 0.75, y: 0.5, pen: true, pressure: 0.4 }, true);
        let mut a = [9f32; crate::types::ANALOG_LEN];
        g.pack_touches(&mut a);
        let t = crate::types::ANALOG_TOUCH;
        let n = TOUCH_FLOATS;
        assert_eq!(a[t..t + n], [1., 0.5, 0.25, 1.]);
        assert_eq!(a[t + n..t + 2 * n], [0.; 4]);
        assert_eq!(a[t + 2 * n..t + 3 * n], [2., 0.75, 0.5, 0.4]);
        assert_eq!(a[..t], [9f32; crate::types::ANALOG_TOUCH], "the mouse is left alone");
    }
}

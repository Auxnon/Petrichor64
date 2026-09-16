use std::borrow::Borrow;

use glam::{vec3, Vec3};

#[cfg(feature = "puc_lua")]
use mlua::{Function, UserData, UserDataFields, UserDataMethods, Value::Nil};
#[cfg(feature = "picc")]
use piccolo::{Function, Value::Nil, Value::UserData};
#[cfg(feature = "silt")]
use silt_lua::userdata::{UserData, UserDataFields, UserDataMethods};
use silt_lua::value::Value;
use silt_lua::LuaError;

//REMEMBER, setting the ent to dirty will hit the entity manager so fast then any other values changed even on the enxt line will be overlooked. The main thread is THAT much faster...
// Serialize/Deserialize let a LuaEnt cross the wasm web-worker postMessage
// boundary (the Spawn message) — see worker_protocol.rs.
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub struct LuaEnt {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub rot_x: f64,
    pub rot_y: f64,
    pub rot_z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub scale: f64,
    id: u64, // pub uuid: String,
    asset: String,
    // ent: Option<Ent>,
    tex: String,
    anim: bool,
    dirty: bool,
    flags: u8,
    pub flipped: bool,
    pub parent: Option<u64>, // pub children: Option<Vec<Arc<Mutex<LuaEnt>>>>,
    pub bundle_id: u8,
    /// Per-axis size multiplier on top of `scale`, so entities can be
    /// rectangular prisms (a piano key, a wall) not just uniform cubes.
    pub size: [f64; 3],
    pub offset: [f64; 3], // pub meta: mlua::Table,
                          // pub sender: Option<Sender<(u8, MainCommmand)>>,
                          // pub cloned: bool,
    /// Collider shape override for the collision system (`hit.*`, `ent:hit`):
    /// 0 = auto (cylinder for a billboard sprite, baked model AABB otherwise),
    /// 1 = box, 2 = cylinder. See `guide/hit.md`.
    pub hit_shape: u8,
    /// Collider half-extents override, local space: `(x, y, z)` for a box, or
    /// `(radius, radius, half_height)` for a cylinder. `[0,0,0]` (the default)
    /// means "use the computed default" instead of this override.
    pub hit_size: [f64; 3],
    /// Collider center offset override, local space, on top of `hit_size`.
    pub hit_offset: [f64; 3],
    /// Vertex-colour tint, rgba 0..1 — multiplies the entity's shaded output.
    /// Default `[1,1,1,1]` (opaque white) is a no-op. See `guide/entity.md`.
    pub tint: [f64; 4],
    /// This entity's own tile grid, if it has one — set by calling `tile`/
    /// `ent:tile(...)` on a grid-less entity, which lazily creates one (see
    /// `TileGrid`, `guide/entity.md`). `Box` keeps this off the common
    /// (grid-less) `LuaEnt` clone path; `None` is one pointer, no allocation.
    pub grid: Option<Box<crate::tile_grid::TileGrid>>,
}
pub mod lua_ent_flags {
    // pub const None: u8 = 0b0;
    pub const TEX: u8 = 0b1;
    pub const ASSET: u8 = 0b10;
    pub const DEAD: u8 = 0b100;
}

/// World-space position, local-space pivot offset, and per-axis scale for
/// entity `lua` — the shared subset of `Ent::build_meta`'s transform (used
/// there, with rotation, in the headed-only `ent.rs`) and of the collision
/// system's default collider (`ent_manager::collider_for`, `collide.rs`,
/// which deliberately ignores rotation — see `guide/hit.md`). Lives here
/// rather than in `ent.rs` because collision must work in a headless build
/// too, and `ent.rs`/`Ent` are `#[cfg(feature = "headed")]`-only. Position
/// and offset are both ×16 to match every other world-space quantity in the
/// engine (tile positions, `Chunk` cells); `size` folds `lua.scale` in.
pub fn render_transform(lua: &LuaEnt) -> (Vec3, Vec3, Vec3) {
    let pos = vec3(lua.x as f32, lua.y as f32, lua.z as f32) * 16.;
    let offset = vec3(
        lua.offset[0] as f32,
        lua.offset[1] as f32,
        lua.offset[2] as f32,
    ) * 16.;
    let s: f32 = lua.scale as f32;
    // Per-axis size lets entities be rectangular prisms, not just cubes.
    let sz = vec3(
        s * lua.size[0] as f32,
        s * lua.size[1] as f32,
        s * lua.size[2] as f32,
    );
    (pos, offset, sz)
}

// #[cfg(feature = "silt")]
// impl UserData for LuaEnt {
//     fn by_meta_method<'a>(
//         &mut self,
//         lua: &mut Lua,
//         method: MetaMethod,
//         inputs: Value<'a>,
//     ) -> Result<Value<'a>> {
//         match method {
//             MetaMethod::ToString => Ok(Value::String(format!(
//                 "[entity {}]",
//                 inputs.get::<LuaEnt>()?.get_id()
//             ))),
//             _ => Ok(Value::Nil),
//         }
//     }
// }

macro_rules! safe_unwrap {
    ($ud:ident) => {
        if let Some(ud) = $ud {
            ud
        } else {
            return Err(LuaError::UDBadCast);
        }
    };
}
// #[cfg(feature = "puc_lua")]
impl UserData for LuaEnt {
    // fn to_string(&self) -> String {
    //     let t = String::from("ent");
    //     return t;
    // }
    fn type_name() -> &'static str {
        "ent"
    }
    fn get_id(&self) -> usize {
        0
    }

    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, _, this_res, _: ()| {
            let this = safe_unwrap!(this_res);
            Ok(format!("[entity {}]", this.get_id()))
        });
        methods.add_meta_method("__concat", |_, _, this_res, _: ()| {
            let this = safe_unwrap!(this_res);
            Ok(format!("[entity {}]", this.get_id()))
        });
        methods.add_method_mut("pos", |_, _, this_res, p: (f64, f64, f64)| {
            let this = safe_unwrap!(this_res);
            this.x = p.0;
            this.y = p.1;
            this.z = p.2;

            Ok(Value::Nil)
        });

        methods.add_method_mut("anim", |_, _, this_res, (tex, force): (String, bool)| {
            let this = safe_unwrap!(this_res);
            if tex != this.tex || force {
                this.dirty = true;
                this.tex = tex;
            }
            this.anim = true;

            Ok(Value::Bool(true))
        });

        methods.add_method_mut("copy", |lua, mc, this_res, _: ()| {
            let this = safe_unwrap!(this_res);
            let ent = this.clone();
            // let wrapped = std::sync::Arc::new(std::sync::Mutex::new(ent));
            let wrapped = lua.create_userdata(mc, ent);
            let arr = vec![wrapped.clone()];
            if let Some(Value::NativeFunction(v)) = lua.globals.borrow().get("_make") {
                v.f.call(lua, mc, &arr);
            }

            // if let Ok(fun) = lua.globals).get::<&str, Function>("_make") {
            //     fun.call(wrapped.clone())?;
            // }
            //

            Ok(wrapped)
        });

        methods.add_method_mut("kill", |_, _, this, ()| Ok(this.unwrap().kill()));

        // This entity's own tile grid — mirrors the world's tile()/dtile()/
        // istile()/gtile()/ftile() natives (command.rs) exactly, just scoped
        // to `self.grid` instead of the world. True colon methods (not a
        // bridge to a channel-backed native): the grid lives on `self`, so
        // there's no main-thread state to reach across to, the same reason
        // `LuaImg`'s `fill`/`rect`/etc. work as plain colon methods. See
        // guide/entity.md and guide/hit.md.
        methods.add_method_mut(
            "tile",
            |_, _, this_res, (asset, x, y, z, rot): (Value, i32, i32, i32, Option<u8>)| {
                let this = safe_unwrap!(this_res);
                let name = match asset {
                    Value::String(s) => s,
                    _ => String::new(),
                };
                let grid = this
                    .grid
                    .get_or_insert_with(|| Box::new(crate::tile_grid::TileGrid::new()));
                grid.set_tile(name, rot.unwrap_or(0), x, y, z);
                Ok(Value::Nil)
            },
        );

        methods.add_method_mut(
            "dtile",
            |_, _, this_res, (x, y, z): (Option<i32>, Option<i32>, Option<i32>)| {
                let this = safe_unwrap!(this_res);
                if let Some(grid) = this.grid.as_mut() {
                    // Unlike the world's `dtile` (command.rs) — which has a
                    // pre-existing dead-code bug (its "no args" match arm is
                    // unreachable, so it never actually clears everything —
                    // out of scope to fix here, flagged separately) — this
                    // implements the documented "no args clears everything"
                    // behavior correctly.
                    match (x, y, z) {
                        (Some(xx), Some(yy), Some(zz)) => grid.drop_chunk(xx, yy, zz),
                        _ => grid.clear(),
                    }
                }
                Ok(Value::Nil)
            },
        );

        methods.add_method_mut("istile", |_, _, this_res, (x, y, z): (i32, i32, i32)| {
            let this = safe_unwrap!(this_res);
            Ok(match &this.grid {
                Some(g) => Value::Bool(g.is_tile(x, y, z)),
                None => Value::Nil,
            })
        });

        methods.add_method_mut("gtile", |_, _, this_res, (x, y, z): (i32, i32, i32)| {
            let this = safe_unwrap!(this_res);
            Ok(match &this.grid {
                Some(g) => Value::String(match g.get_tile(x, y, z) {
                    Some((name, _)) => name,
                    None => String::new(),
                }),
                None => Value::Nil,
            })
        });

        methods.add_method_mut(
            "ftile",
            |lua,
             mc,
             this_res,
             (t, x, y, z, dx, dy, dz): (String, i32, i32, i32, i32, i32, i32)| {
                let this = safe_unwrap!(this_res);
                match &this.grid {
                    Some(g) => {
                        let tt = if t.is_empty() { None } else { Some(t.as_str()) };
                        match g.first_tile(tt, x, y, z, dx, dy, dz, 100) {
                            Some(v) => Ok(lua.table_from_array(
                                mc,
                                vec![v[0] as f64, v[1] as f64, v[2] as f64],
                            )),
                            None => {
                                let t = lua.raw_table();
                                Ok(lua.wrap_table(mc, t))
                            }
                        }
                    }
                    None => Ok(Value::Nil),
                }
            },
        );
    }

    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, _, this| this.x);
        fields.add_field_method_set("x", |_, _, this: &mut Self, x: f64| this.x = x);

        fields.add_field_method_get("y", |_, _, this| this.y);
        fields.add_field_method_set("y", |_, _, this, y: f64| this.y = y);

        fields.add_field_method_get("z", |_, _, this| this.z);
        fields.add_field_method_set("z", |_, _, this, z: f64| this.z = z);

        fields.add_field_method_get("rx", |_, _, this| this.rot_x);
        fields.add_field_method_get("ry", |_, _, this| this.rot_y);
        fields.add_field_method_get("rz", |_, _, this| this.rot_z);

        fields.add_field_method_set("rz", |_, _, this, rot_z: f64| this.rot_z = rot_z);
        fields.add_field_method_set("ry", |_, _, this, rot_y: f64| this.rot_y = rot_y);
        fields.add_field_method_set("rx", |_, _, this, rot_x: f64| this.rot_x = rot_x);

        fields.add_field_method_get("vx", |_, _, this| this.vx);
        fields.add_field_method_set("vx", |_, _, this, vx: f64| this.vx = vx);
        fields.add_field_method_get("vy", |_, _, this| this.vy);
        fields.add_field_method_set("vy", |_, _, this, vy: f64| this.vy = vy);
        fields.add_field_method_get("vz", |_, _, this| this.vz);
        fields.add_field_method_set("vz", |_, _, this, vz: f64| this.vz = vz);

        fields.add_field_method_get("flipped", |_, _, this| this.flipped);
        fields.add_field_method_set("flipped", |_, _, this, flipped: bool| {
            this.flipped = flipped
        });

        fields.add_field_method_get("offset", |_, _, this| Ok(this.offset));
        fields.add_field_method_set("offset", |_, _, this, offset: [f64; 3]| {
            Ok(this.offset = offset)
        });

        // Per-axis size (x, y, z) for rectangular-prism entities.
        fields.add_field_method_get("size", |_, _, this| Ok(this.size));
        fields.add_field_method_set("size", |_, _, this, size: [f64; 3]| Ok(this.size = size));

        // Collider override for the collision system — see guide/hit.md.
        fields.add_field_method_get("hit_shape", |_, _, this| Ok(this.hit_shape));
        fields.add_field_method_set("hit_shape", |_, _, this, hit_shape: u8| {
            Ok(this.hit_shape = hit_shape)
        });
        fields.add_field_method_get("hit_size", |_, _, this| Ok(this.hit_size));
        fields.add_field_method_set("hit_size", |_, _, this, hit_size: [f64; 3]| {
            Ok(this.hit_size = hit_size)
        });
        fields.add_field_method_get("hit_offset", |_, _, this| Ok(this.hit_offset));
        fields.add_field_method_set("hit_offset", |_, _, this, hit_offset: [f64; 3]| {
            Ok(this.hit_offset = hit_offset)
        });

        // Vertex-colour tint, rgba 0..1 — see guide/entity.md.
        fields.add_field_method_get("tint", |_, _, this| Ok(this.tint));
        fields.add_field_method_set("tint", |_, _, this, tint: [f64; 4]| Ok(this.tint = tint));

        fields.add_field_method_set("scale", |_, _, this, scale: f64| Ok(this.scale = scale));

        fields.add_field_method_get("id", |_, _, this| Ok(this.id));
        fields.add_field_method_get("tex", |_, _, this| Ok(this.tex.clone()));
        fields.add_field_method_set("tex", |_, _, this, tex: String| {
            if this.tex != tex {
                this.tex = tex;
                this.dirty = true;
                this.flags |= lua_ent_flags::TEX;
            } else if this.anim {
                this.anim = false;
                this.dirty = true;
                this.flags |= lua_ent_flags::TEX;
            }
            Ok(())
        });
        fields.add_field_method_get("asset", |_, _, this| Ok(this.asset.clone()));
        fields.add_field_method_set("asset", |_, _, this, asset: String| {
            if this.asset != asset {
                this.asset = asset;
                this.flags |= lua_ent_flags::ASSET;
                this.dirty = true;
            }
            Ok(())
        });
    }
}

// impl Drop for LuaEnt {
//     fn drop(&mut self) {
//         println!("dropping lua ent {} && cloned is {}", self.id, self.cloned);
//         self.dirty = true;
//         self.dead = true;
//     }
// }
// impl serde::Serialize for LuaEnt {
//     fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
//     where
//         S: serde::Serializer {
//         todo!()
//     }
// }

impl LuaEnt {
    // pub fn empty() -> LuaEnt {
    //     LuaEnt {
    //         x: 0.,
    //         y: 0.,
    //         z: 0.,
    //         rot_x: 0.,
    //         rot_y: 0.,
    //         rot_z: 0.,
    //         vx: 0.,
    //         vy: 0.,
    //         vz: 0.,
    //         id: 0,
    //         scale: 1.,
    //         // ent: None,
    //         asset: String::new(),
    //         tex: String::new(),
    //         dirty: false,
    //         anim: false,
    //         dead: false,
    //         flipped: false,
    //         parent: None, // children: None,
    //         bundle_id: 0,
    //     }
    // }

    pub fn new(
        // sender: Sender<(u8, MainCommmand)>,
        id: u64,
        asset: String,
        x: f64,
        y: f64,
        z: f64,
        scale: f64,
    ) -> LuaEnt {
        LuaEnt {
            // sender: Some(sender),
            id,
            x,
            y,
            z,
            rot_x: 0.,
            rot_y: 0.,
            rot_z: 0.,
            vx: 0.,
            vy: 0.,
            vz: 0.,
            scale,
            // ent: None,
            asset,
            tex: String::new(),
            dirty: false,
            anim: false,
            flipped: false,
            parent: None, // children: None,
            bundle_id: 0,
            size: [1., 1., 1.],
            offset: [0., 0., 0.], // meta: mlua::Table::new(),
            flags: 0,
            // cloned: false,
            hit_shape: 0,
            hit_size: [0., 0., 0.],
            hit_offset: [0., 0., 0.],
            tint: [1., 1., 1., 1.],
            grid: None,
        }
    }
    // pub fn set_id(&mut self, id: u64) {
    //     self.id = id;
    // }
    pub fn get_id(&self) -> u64 {
        // https://stackoverflow.com/questions/39638363/how-can-i-use-a-hashmap-with-f64-as-key-in-rust
        self.id
    }
    pub fn get_asset(&self) -> String {
        self.asset.clone()
    }
    pub fn get_tex(&self) -> &String {
        &self.tex
    }
    pub fn get_flags(&self) -> u8 {
        self.flags
    }
    pub fn is_dead(&self) -> bool {
        self.flags & lua_ent_flags::DEAD != 0
    }
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn is_anim(&self) -> bool {
        self.anim
    }
    pub fn clear_dirt(&mut self) {
        self.dirty = false;
        self.flags = 0;
    }
    pub fn kill(&mut self) {
        self.flags |= lua_ent_flags::DEAD;
        self.dirty = true;
    }
}

impl Clone for LuaEnt {
    fn clone(&self) -> LuaEnt {
        // MARK by clonging luaents for renderer and then dropping hte clones we're calling the deconstrcutor so this wont work
        // Do we even need to clone lua ents? Can we just pass a reference to the lua ent?
        // This was done to avoid having the lua ent lock up the render frame because the lua context is mutating it.
        // We;re avoiding teh Arc consequences but is it worth it
        LuaEnt {
            x: self.x,
            y: self.y,
            z: self.z,
            vx: self.vx,
            vy: self.vy,
            vz: self.vz,
            rot_x: self.rot_x,
            rot_y: self.rot_y,
            rot_z: self.rot_z,
            id: self.id,
            scale: self.scale,
            // ent: None,
            asset: self.asset.clone(),
            tex: self.tex.clone(),
            dirty: true,
            anim: self.anim,
            flipped: self.flipped,
            parent: self.parent, // children,
            bundle_id: self.bundle_id,
            size: self.size,
            offset: self.offset,
            flags: self.flags,
            // meta: self.meta.clone(),
            // sender: None,
            // cloned: true,
            hit_shape: self.hit_shape,
            hit_size: self.hit_size,
            hit_offset: self.hit_offset,
            tint: self.tint,
            // Deep clone, same as everything else here — cheap when None
            // (the common case), and grids are expected to be moving-
            // platform-sized (small, sparse), not world-sized, so this
            // isn't expected to matter; revisit if it measures otherwise.
            grid: self.grid.clone(),
        }
    }
}

impl ToString for LuaEnt {
    fn to_string(&self) -> String {
        format!(
            "entity(id: {}, asset: {}, tex: {}, x: {}, y: {}, z: {}, vx: {}, vy: {}, vz: {}, rot_x: {}, rot_y: {}, rot_z: {}, scale: {}, dirty: {}, anim: {}, flipped: {}, parent: {:?}, bundle_id: {}, offset: {:?}, flags: {})",
            self.id, self.asset, self.tex, self.x, self.y, self.z, self.vx, self.vy, self.vz, self.rot_x, self.rot_y, self.rot_z, self.scale, self.dirty, self.anim, self.flipped, self.parent.is_some(), self.bundle_id, self.offset, self.flags
        )
    }
}

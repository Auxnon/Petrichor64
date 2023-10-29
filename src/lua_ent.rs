#[cfg(feature = "puc_lua")]
use mlua::{Function, UserData, UserDataFields, UserDataMethods, Value::Nil};
#[cfg(feature = "picc")]
use piccolo::{Function, UserDataFields, UserDataMethods, Value::Nil, Value::UserData};
#[cfg(feature = "silt")]
use silt_lua::prelude::{Lua, MetaMethod, UserData, Value};

//REMEMBER, setting the ent to dirty will hit the entity manager so fast then any other values changed even on the enxt line will be overlooked. The main thread is THAT much faster...
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
    pub offset: [f64; 3], // pub meta: mlua::Table,
                          // pub sender: Option<Sender<(u8, MainCommmand)>>,
                          // pub cloned: bool,
}
pub mod lua_ent_flags {
    // pub const None: u8 = 0b0;
    pub const TEX: u8 = 0b1;
    pub const ASSET: u8 = 0b10;
    pub const DEAD: u8 = 0b100;
}

#[cfg(feature = "silt")]
impl UserData<'_> for LuaEnt {
    fn by_meta_method<'a>(
        &mut self,
        lua: &mut Lua,
        method: MetaMethod,
        inputs: Value<'a>,
    ) -> Result<Value<'a>> {
        match method {
            MetaMethod::ToString => Ok(Value::String(format!(
                "[entity {}]",
                inputs.get::<LuaEnt>()?.get_id()
            ))),
            _ => Ok(Value::Nil),
        }
    }
}
#[cfg(feature = "puc_lua")]
impl UserData for LuaEnt {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_meta_method("__tostring", |_, this, _: ()| {
            Ok(format!("[entity {}]", this.get_id()))
        });
        methods.add_meta_method("__concat", |_, this, _: ()| {
            Ok(format!("[entity {}]", this.get_id()))
        });
        methods.add_method_mut("pos", |_, this, p: (f64, f64, f64)| {
            this.x = p.0;
            this.y = p.1;
            this.z = p.2;

            Ok(())
        });

        methods.add_method_mut("anim", |_, this, (tex, force): (String, Option<bool>)| {
            if tex != this.tex || force.unwrap_or(false) {
                this.dirty = true;
                this.tex = tex;
            }
            this.anim = true;

            Ok(true)
        });

        methods.add_method_mut("copy", |lua, this, ()| {
            let ent = this.clone();
            let wrapped = std::sync::Arc::new(std::sync::Mutex::new(ent));
            if let Ok(fun) = lua.globals().get::<&str, Function>("_make") {
                fun.call(wrapped.clone())?;
            }

            Ok(wrapped)
        });

        methods.add_method_mut("kill", |_, this, ()| {
            this.kill();
            Ok(Nil)
        });
    }

    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, this| Ok(this.x));
        fields.add_field_method_set("x", |_, this, x: f64| Ok(this.x = x));

        fields.add_field_method_get("y", |_, this| Ok(this.y));
        fields.add_field_method_set("y", |_, this, y: f64| Ok(this.y = y));

        fields.add_field_method_get("z", |_, this| Ok(this.z));
        fields.add_field_method_set("z", |_, this, z: f64| Ok(this.z = z));

        fields.add_field_method_get("rx", |_, this| Ok(this.rot_x));
        fields.add_field_method_get("ry", |_, this| Ok(this.rot_y));
        fields.add_field_method_get("rz", |_, this| Ok(this.rot_z));

        fields.add_field_method_set("rz", |_, this, rot_z: f64| Ok(this.rot_z = rot_z));
        fields.add_field_method_set("ry", |_, this, rot_y: f64| Ok(this.rot_y = rot_y));
        fields.add_field_method_set("rx", |_, this, rot_x: f64| Ok(this.rot_x = rot_x));

        fields.add_field_method_get("vx", |_, this| Ok(this.vx));
        fields.add_field_method_set("vx", |_, this, vx: f64| Ok(this.vx = vx));
        fields.add_field_method_get("vy", |_, this| Ok(this.vy));
        fields.add_field_method_set("vy", |_, this, vy: f64| Ok(this.vy = vy));
        fields.add_field_method_get("vz", |_, this| Ok(this.vz));
        fields.add_field_method_set("vz", |_, this, vz: f64| Ok(this.vz = vz));

        fields.add_field_method_get("flipped", |_, this| Ok(this.flipped));
        fields.add_field_method_set("flipped", |_, this, flipped: bool| {
            // println!("flipped it {}", flipped);
            Ok(this.flipped = flipped)
        });

        fields.add_field_method_get("offset", |_, this| Ok(this.offset));
        fields.add_field_method_set("offset", |_, this, offset: [f64; 3]| {
            Ok(this.offset = offset)
        });

        fields.add_field_method_set("scale", |_, this, scale: f64| Ok(this.scale = scale));

        fields.add_field_method_get("id", |_, this| Ok(this.id));
        fields.add_field_method_get("tex", |_, this| Ok(this.tex.clone()));
        fields.add_field_method_set("tex", |_, this, tex: String| {
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
        fields.add_field_method_get("asset", |_, this| Ok(this.asset.clone()));
        fields.add_field_method_set("asset", |_, this, asset: String| {
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
            offset: [0., 0., 0.], // meta: mlua::Table::new(),
            flags: 0,
            // cloned: false,
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
            offset: self.offset,
            flags: self.flags,
            // meta: self.meta.clone(),
            // sender: None,
            // cloned: true,
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

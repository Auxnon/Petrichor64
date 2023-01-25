use std::sync::mpsc::Sender;

use mlua::{UserData, UserDataFields, UserDataMethods, Value::Nil};

use crate::command::MainCommmand;
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
    pub flipped: bool,
    pub dead: bool,
    pub parent: Option<u64>, // pub children: Option<Vec<Arc<Mutex<LuaEnt>>>>,
    pub bundle_id: u8,
    // pub sender: Option<Sender<(u8, MainCommmand)>>,
    // pub cloned: bool,
}

impl UserData for LuaEnt {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        // methods.add_method_mut(name, method)
        // set meta method __gc
        // let h = mlua::MetaMethod::
        methods.add_meta_method("__tostring", |lu, this, _: ()| {
            // println!("lua ent gc'd");
            // match lu.globals().get::<&str, mlua::Function>("kill") {
            //     Ok(kill) => {
            //         kill.call::<_, ()>((this.get_id(),))?;
            //     }
            //     Err(e) => {
            //         println!("kill error: {}", e);
            //     }
            // }
            Ok(format!("[entity {}]", this.get_id()))
        });
        //mlua::MetaMethod::Concat::name()
        methods.add_meta_method("__concat", |lu, this, _: ()| {
            Ok(format!("[entity {}]", this.get_id()))
        });
        methods.add_method_mut("pos", |_, this, p: (f64, f64, f64)| {
            this.x = p.0;
            this.y = p.1;
            this.z = p.2;

            Ok(())
        });

        methods.add_method_mut("stex", |_, this, tex: String| {
            if this.tex != tex {
                this.tex = tex;
                this.dirty = true;
            } else if this.anim {
                this.anim = false;
                this.dirty = true;
            }

            Ok(true)
        });
        methods.add_method_mut("anim", |_, this, (tex, force): (String, Option<bool>)| {
            let t = this.tex.clone();
            this.tex = tex;
            this.anim = true;
            match force {
                Some(f) => this.dirty = f,
                None => {
                    if t != this.tex {
                        this.dirty = true;
                        // println!("lua current anim {} and is now {}", this.tex, this.dirty);
                    }
                }
            }

            Ok(true)
        });

        methods.add_method("is_dirty", |_, this, ()| Ok(this.dirty));
        methods.add_method("get_tex", |_, this, ()| Ok(this.tex.clone()));

        methods.add_method("get_y", |_, this, ()| Ok(this.y));
        methods.add_method("kill", |lu, this, ()| {
            // lu.call_function::<_, ()>("kill_ent", (this.get_id(),))?;

            //             lua.load(r#"
            //     assert(myobject.val == 123)
            //     myobject:add(7)
            //     assert(myobject.val == 130)
            //     assert(myobject + 10 == 140)
            // "#).exec()?;
            // lu.load(&format!("kill(1)", this.get_id())).exec()?;
            match lu.globals().get::<&str, mlua::Function>("kill") {
                Ok(kill) => {
                    kill.call::<_, ()>((this.get_id(),))?;
                }
                Err(e) => {
                    println!("kill error: {}", e);
                }
            }
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

        fields.add_field_method_set("scale", |_, this, scale: f64| Ok(this.scale = scale));

        fields.add_field_method_get("id", |_, this| Ok(this.id));
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
            dead: false,
            flipped: false,
            parent: None, // children: None,
            bundle_id: 0,
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
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }
    pub fn is_anim(&self) -> bool {
        self.anim
    }
    pub fn clear_dirt(&mut self) {
        self.dirty = false;
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
            dirty: false,
            anim: self.anim,
            dead: false,
            flipped: self.flipped,
            parent: self.parent, // children,
            bundle_id: self.bundle_id,
            // sender: None,
            // cloned: true,
        }
    }
}

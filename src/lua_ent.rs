use crate::Ent;

use mlua::{UserData, UserDataFields, UserDataMethods};
pub struct LuaEnt {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rot_x: f32,
    pub rot_y: f32,
    pub rot_z: f32,
    pub vel_x: f32,
    pub vel_y: f32,
    pub vel_z: f32,
    // pub uuid: String,
}

impl UserData for LuaEnt {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        methods.add_method("get_x", |_, this, ()| Ok(this.x));
        methods.add_method("get_y", |_, this, ()| Ok(this.y));
    }
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("x", |_, this| Ok(this.x));
        fields.add_field_method_set("x", |_, this, x: f32| Ok(this.x = x));

        fields.add_field_method_get("y", |_, this| Ok(this.y));
        fields.add_field_method_set("y", |_, this, y: f32| Ok(this.y = y));

        fields.add_field_method_get("z", |_, this| Ok(this.z));
        fields.add_field_method_set("z", |_, this, z: f32| Ok(this.z = z));

        fields.add_field_method_get("rot_x", |_, this| Ok(this.rot_x));
        fields.add_field_method_get("rot_y", |_, this| Ok(this.rot_y));
        fields.add_field_method_get("rot_z", |_, this| Ok(this.rot_z));

        fields.add_field_method_set("rot_z", |_, this, rot_z: f32| Ok(this.rot_z = rot_z));
        fields.add_field_method_set("rot_y", |_, this, rot_y: f32| Ok(this.rot_y = rot_y));
        fields.add_field_method_set("rot_x", |_, this, rot_x: f32| Ok(this.rot_x = rot_x));

        fields.add_field_method_get("vel_x", |_, this| Ok(this.vel_x));
        fields.add_field_method_set("vel_x", |_, this, vel_x: f32| Ok(this.vel_x = vel_x));
        fields.add_field_method_get("vel_y", |_, this| Ok(this.vel_y));
        fields.add_field_method_set("vel_y", |_, this, vel_y: f32| Ok(this.vel_y = vel_y));
        fields.add_field_method_get("vel_z", |_, this| Ok(this.vel_z));
        fields.add_field_method_set("vel_z", |_, this, vel_z: f32| Ok(this.vel_z = vel_z));
    }
}
impl LuaEnt {
    pub fn empty() -> LuaEnt {
        LuaEnt {
            x: 0.,
            y: 0.,
            z: 0.,
            rot_x: 0.,
            rot_y: 0.,
            rot_z: 0.,
            vel_x: 0.,
            vel_y: 0.,
            vel_z: 0.,
        }
    }
}

// pub fn new() -> LuaEnt {
//     return LuaEnt { x: 10., y: 12. };
// }
//methods.add_method("add_x", |_, this, ()| Ok(Self.ent.set_x(10.)));

// methods.add_meta_function(MetaMethod::Add, |_, (vec1, vec2): (Vec2, Vec2)| {
//     Ok(Vec2(vec1.0 + vec2.0, vec1.1 + vec2.1))
// });

impl Clone for LuaEnt {
    fn clone(&self) -> LuaEnt {
        LuaEnt {
            x: self.x,
            y: self.y,
            z: self.z,
            vel_x: self.vel_x,
            vel_y: self.vel_y,
            vel_z: self.vel_z,
            rot_x: self.rot_x,
            rot_y: self.rot_y,
            rot_z: self.rot_z,
        }
    }
}

impl LuaEnt {
    fn new(ent: Ent) -> LuaEnt {
        LuaEnt {
            x: ent.pos.x,
            y: ent.pos.y,
            z: ent.pos.z,
            vel_x: ent.vel.x,
            vel_y: ent.vel.y,
            vel_z: ent.vel.z,
            rot_x: ent.rot.x,
            rot_y: ent.rot.y,
            rot_z: ent.rot.z,
        }
    }
}

// impl<T: IAnimalData> Animal<T> {
// impl<'b> UserData for LuaEnt<'b> {
//     fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
//         methods.add_method("add_x", |_, this, ()| Ok(Self.ent.set_x(10.)));

//         methods.add_async_function(
//             "read",
//             |lua, (this, size): (AnyUserData, usize)| async move {
//                 let mut this = this.borrow_mut::<Self>()?;
//                 let mut buf = vec![0; size];
//                 let n = this.0.read(&mut buf).await?;
//                 buf.truncate(n);
//                 lua.create_string(&buf)
//             },
//         );

//         methods.add_async_function(
//             "write",
//             |_, (this, data): (AnyUserData, LuaString)| async move {
//                 let mut this = this.borrow_mut::<Self>()?;
//                 let n = this.0.write(&data.as_bytes()).await?;
//                 Ok(n)
//             },
//         );

//         methods.add_async_function("close", |_, this: AnyUserData| async move {
//             let mut this = this.borrow_mut::<Self>()?;
//             this.0.shutdown().await?;
//             Ok(())
//         });
//     }
// }

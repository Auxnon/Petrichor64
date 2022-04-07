use std::sync::Arc;

use glam::vec4;
use mlua::{Error, Lua, Table};
use parking_lot::RwLock;

use crate::{ent_master, global::Global, switch_board::SwitchBoard};

pub fn init_lua_sys(lua_ctx: &Lua, lua_globals: &Table, switch_board: Arc<RwLock<SwitchBoard>>) {
    println!("init lua sys");

    let default_func = lua_ctx
        .create_function(|_, e: f32| Ok("placeholder func uwu"))
        .unwrap();
    res(lua_globals.set("_default_func", default_func));

    let multi = lua_ctx.create_function(|_, (x, y): (f32, f32)| Ok(x * y));
    lua_globals.set("multi", multi.unwrap());

    res(lua_globals.set(
        "_time",
        lua_ctx.create_function(|_, (): ()| Ok(17)).unwrap(),
    ));

    res(lua_globals.set(
        "_point",
        lua_ctx
            .create_function(|_, (): ()| {
                let mut mutex = crate::ent_master.lock();
                let entity_manager = mutex.get_mut().unwrap();
                if entity_manager.entities.len() > 0 {
                    let p = entity_manager.entities[0].pos;
                    Ok((p.x, p.y, p.z))
                } else {
                    Ok((0., 0., 0.))
                }
            })
            .unwrap(),
    ));

    res(lua_globals.set(
        "_bg",
        lua_ctx
            .create_function(move |_, (x, y, z, w): (f32, f32, f32, f32)| {
                let mut mutex = &mut switch_board.write();
                mutex.background = vec4(x, y, z, w);
                // parking_lot::RwLockWriteGuard::unlock_fair(*mutex);
                Ok(1)
            })
            .unwrap(),
    ));

    res(lua_globals.set(
        "_spawn",
        lua_ctx
            .create_function(|_, (x, y, z): (f32, f32, f32)| {
                let mut mutex = crate::ent_master.lock();
                let manager = mutex.get_mut().unwrap();
                let l = manager.add(x, y, z);
                Ok(l)
            })
            .unwrap(),
    ));

    res(lua_globals.set(
        "_self_destruct",
        lua_ctx
            .create_function(|_, (): ()| {
                Ok(())
                //
            })
            .unwrap(),
    ));
}
fn res(r: Result<(), Error>) {
    let st = "🔴lua::problem setting default lua functions";
    println!("{}", st);
    crate::log::log(st.to_string());
}

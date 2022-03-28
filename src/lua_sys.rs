use mlua::{Error, Lua, Table};

use crate::ent_master;

pub fn init_lua_sys(lua_ctx: &Lua, globals: &Table) {
    println!("init lua sys");

    let default_func = lua_ctx
        .create_function(|_, e: f32| Ok("placeholder func uwu"))
        .unwrap();
    res(globals.set("_default_func", default_func));

    let multi = lua_ctx.create_function(|_, (x, y): (f32, f32)| Ok(x * y));
    globals.set("multi", multi.unwrap());

    res(globals.set(
        "_time",
        lua_ctx.create_function(|_, (): ()| Ok(17)).unwrap(),
    ));

    res(globals.set(
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

    res(globals.set(
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

    res(globals.set(
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

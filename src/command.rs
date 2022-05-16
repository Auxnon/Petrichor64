use std::sync::Arc;

use glam::vec4;
use mlua::{Error, Lua, Table};
use parking_lot::RwLock;

use crate::{lua_ent::LuaEnt, switch_board::SwitchBoard, Core};

/** Private commands not reachable by lua code, but also works without lua being loaded */
pub fn init_con_sys(core: &Core, s: &String) -> bool {
    if s.len() <= 0 {
        return false;
    }
    let segments = s.split(" ").collect::<Vec<&str>>();
    match segments[0] {
        "$die" => {
            // this chunk could probably be passed directly to lua core but being it's significance it felt important to pass into our pre-system check for commands
            let guard = crate::lua_master.lock();
            let lua_core = guard.get();
            if lua_core.is_some() {
                lua_core.unwrap().die();
            }
        }
        "$pack" => {
            crate::asset::pack(&if segments.len() > 1 {
                format!("{}.game.png", segments[1])
            } else {
                "game.png".to_string()
            });
        }
        "$unpack" => {
            //
            crate::zip_pal::unpack_and_save(&"biggo.png".to_string(), &"biggo.zip".to_string());
        }
        "$load" => {
            let mutex = crate::lua_master.lock();
            match mutex.get() {
                Some(d) => {}
                None => {
                    crate::texture::reset();
                    let lua_ref = mutex.get_or_init(|| {
                        crate::lua_define::LuaCore::new(Arc::clone(&core.switch_board))
                        //pollster::block_on(
                    });
                    std::mem::drop(mutex);
                    // println!("thread sleep...");
                    // std::thread::sleep(std::time::Duration::from_millis(1000));
                    // println!("thread slept")

                    if segments.len() > 1 {
                        crate::asset::unpack(&core.device, &format!("{}.game.png", segments[1]));
                    } else {
                        crate::asset::walk_files(Some(&core.device));
                    }

                    // lua_ref.call_main();
                    // crate::texture::reset();
                    // crate::asset::init(&core.device);

                    let mut mutex = crate::ent_master.lock();
                    let entity_manager = mutex.get_mut().unwrap();
                    crate::texture::refinalize(&core.device, &core.queue, &core.master_texture);
                    for e in &mut entity_manager.entities {
                        e.hot_reload();
                    }
                    let mutex = crate::lua_master.lock();
                    mutex.get().unwrap().call_main();
                    log("buldozed into this here code with a buncha stuff".to_string());
                }
            }
        }
        "$print_atlas" => {
            crate::texture::save_atlas();
        }
        "$ugh" => {
            //
        }
        &_ => return false,
    }
    true
}

pub fn init_lua_sys(
    lua_ctx: &Lua,
    lua_globals: &Table,
    switch_board: Arc<RwLock<SwitchBoard>>,
) -> Result<(), Error> {
    println!("init lua sys");

    let default_func = lua_ctx
        .create_function(|_, e: f32| Ok("placeholder func uwu"))
        .unwrap();
    res(lua_globals.set("_default_func", default_func));

    let multi = lua_ctx.create_function(|_, (x, y): (f32, f32)| Ok(x * y));
    lua_globals.set("multi", multi.unwrap());

    lua_globals.set("_ents", lua_ctx.create_table()?);

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

    let switch = Arc::clone(&switch_board);
    res(lua_globals.set(
        "_bg",
        lua_ctx
            .create_function(move |_, (x, y, z, w): (f32, f32, f32, f32)| {
                let mut mutex = &mut switch.write();
                mutex.background = vec4(x, y, z, w);
                // parking_lot::RwLockWriteGuard::unlock_fair(*mutex);
                Ok(1)
            })
            .unwrap(),
    ));

    let switch = Arc::clone(&switch_board);
    res(lua_globals.set(
        "_tile",
        lua_ctx
            .create_function(move |_, (t, x, y, z): (f32, f32, f32, f32)| {
                // core.world.set_tile(format!("grid"), 0, 0, 16 * 0);
                let mut mutex = &mut switch.write();
                mutex.tile_queue.push(vec4(t, x, y, z));
                // let mut mutex = &mut switch_board.write();
                // mutex.background = vec4(x, y, z, w);
                // parking_lot::RwLockWriteGuard::unlock_fair(*mutex);
                Ok(1)
            })
            .unwrap(),
    ));
    let switch = Arc::clone(&switch_board);
    res(lua_globals.set(
        "_tile_done",
        lua_ctx
            .create_function(move |_, (): ()| {
                let mut mutex = &mut switch.write();
                mutex.dirty = true;
                Ok(1)
            })
            .unwrap(),
    ));
    let switch = Arc::clone(&switch_board);
    res(lua_globals.set(
        "_prt",
        lua_ctx
            .create_function(
                move |_,
                      (tex, n, x, y, z, vx, vy, vz): (
                    String,
                    f32,
                    f32,
                    f32,
                    f32,
                    f32,
                    f32,
                    f32,
                )| {
                    let mut mutex = &mut switch.write();
                    mutex.dirty = true;
                    Ok(1)
                },
            )
            .unwrap(),
    ));
    let switch = Arc::clone(&switch_board);

    res(lua_globals.set(
        "_spawn",
        lua_ctx
            .create_function(move |lua, (x, y, z): (f32, f32, f32)| {
                // pub fn add(&mut self, x: f32, y: f32, z: f32) -> LuaEnt {
                let mut ent = crate::lua_ent::LuaEnt::empty();
                ent.x = x;
                ent.y = y;
                ent.z = z;

                // let globl = lua_ctx.globals();
                // lua_globals.set("f", lua_ctx.create_table()?);

                let ents = lua.globals().get::<&str, Table>("_ents")?;
                let index = ents.len()? + 1;
                ents.set(index, ent);
                // let mut mutex = &mut switch.write();`
                // mutex.ent_queue.push(&ent);

                // self.create.push(ent.clone());

                // }

                // let mut mutex = crate::ent_master.lock();
                // let manager = mutex.get_mut().unwrap();
                // let l = manager.add(x, y, z);
                println!("we made an ent");
                // Ok((ents.get::<i64, LuaEnt>(index)))
                Ok(index)
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

    Ok(())
}
fn log(str: String) {
    crate::log::log(format!("com::{}", str));
    println!("{}", str);
}
fn res(r: Result<(), Error>) {
    let st = "🔴lua::problem setting default lua functions";
    println!("{}", st);
    crate::log::log(st.to_string());
}

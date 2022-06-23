use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{
        mpsc::{sync_channel, Sender, SyncSender},
        Arc,
    },
};

use glam::vec4;
use itertools::Itertools;
use mlua::{Error, Lua, Table};
use parking_lot::RwLock;
use std::sync::Mutex;
use winit::event::VirtualKeyCode;

use crate::{lua_ent::LuaEnt, switch_board::SwitchBoard, Core};

/** Private commands not reachable by lua code, but also works without lua being loaded */
pub fn init_con_sys(core: &mut Core, s: &String) -> bool {
    if s.len() <= 0 {
        return false;
    }
    let segments = s.split(" ").collect::<Vec<&str>>();

    match segments[0] {
        "$die" => {
            // this chunk could probably be passed directly to lua core but being it's significance it felt important to pass into our pre-system check for commands
            core.lua_master.die();
        }
        "$pack" => {
            crate::asset::pack(
                &if segments.len() > 1 {
                    format!("{}.game.png", segments[1])
                } else {
                    "game.png".to_string()
                },
                &core.lua_master,
            );
        }
        "$superpack" => {
            log(crate::asset::super_pack(&if segments.len() > 1 {
                format!("{}", segments[1])
            } else {
                "game".to_string()
            })
            .to_string());
        }
        "$unpack" => {
            //
            crate::zip_pal::unpack_and_save(&"biggo.png".to_string(), &"biggo.zip".to_string());
        }
        "$load" => {
            load(
                core,
                if segments.len() > 1 {
                    Some(segments[1].to_string())
                } else {
                    None
                },
            );
        }
        "$reset" => reset(core),
        "$reload" => reload(core),
        "$atlas" => {
            crate::texture::save_atlas();
        }
        "$ls" => {
            let s = if segments.len() > 1 {
                segments[1].to_string().clone()
            } else {
                ".".to_string()
            };
            let path = Path::new(&s);

            match path.read_dir() {
                Ok(read) => {
                    let dir = read
                        .filter(Result::is_ok)
                        .map(|e| format!("{:?}", e.unwrap().path()))
                        .join(",");
                    log(dir);
                }
                Err(er) => {
                    log(format!("returned {}", er));
                }
            }
        }
        "$ugh" => {
            // TODO ugh?
        }
        "$clear" => crate::log::clear(),
        "$test" => {
            log("that test worked, yipee".to_string());
        }
        &_ => return false,
    }
    true
}

pub fn init_lua_sys(
    lua_ctx: &Lua,
    lua_globals: &Table,
    switch_board: Arc<RwLock<SwitchBoard>>,
    pitcher: Sender<(i32, String, i32, i32, i32, SyncSender<i32>)>,
) -> Result<(), Error> {
    println!("init lua sys");

    let default_func = lua_ctx
        .create_function(|_, e: f32| Ok("placeholder func uwu"))
        .unwrap();
    res(
        "_default_func",
        lua_globals.set("_default_func", default_func),
    );

    let multi = lua_ctx.create_function(|_, (x, y): (f32, f32)| Ok(x * y));
    lua_globals.set("multi", multi.unwrap());

    lua_globals.set("_ents", lua_ctx.create_table()?);

    #[macro_export]
    macro_rules! lua {
        ($name:expr,$closure:expr,$desc:expr) => {
            // println!("hiya sailor, it's {}", $name);
            res(
                $name,
                lua_globals.set($name, lua_ctx.create_function($closure).unwrap()),
            );

            // fn $func_name() {
            //     // The `stringify!` macro converts an `ident` into a string.
            //     println!("You called {:?}()",
            //              stringify!($func_name));
            // }
        };
    }

    lua!("time", |_, (): ()| Ok(17), "Get the time.");

    lua!(
        "point",
        |_, (): ()| {
            // let mut mutex = crate::ent_master.lock();
            // let entity_manager = mutex.get_mut().unwrap();
            // if entity_manager.entities.len() > 0 {
            //     let p = entity_manager.entities[0].pos;
            //     Ok((p.x, p.y, p.z))
            // } else {
            Ok((0., 0., 0.))
            // }
        },
        "Get a point"
    );

    lua!(
        "log",
        |_, s: String| {
            log(format!("log::{}", s));
            Ok(())
        },
        "Prints string to console"
    );

    lua!(
        "push",
        move |lua, (n): (f64)| {
            // let ents = lua.globals().get::<&str, Table>("_ents")?;
            // ents.macro_export

            let mut guard = crate::ent_master.write();
            let eman = guard.get_mut().unwrap();

            let ents = &eman.ent_table;
            for wrapped_ent in &mut ents.iter() {
                let mut eg = wrapped_ent.lock().unwrap();
                eg.x += n;
            }

            Ok(())
        },
        "Pushes entities"
    );

    let switch = Arc::clone(&switch_board);
    lua!(
        "bg",
        move |_, (x, y, z, w): (f32, f32, f32, f32)| {
            let mut mutex = &mut switch.write();
            mutex.background = vec4(x, y, z, w);
            // parking_lot::RwLockWriteGuard::unlock_fair(*mutex);
            Ok(1)
        },
        "Get background color"
    );

    // res(lua_globals.set(
    //     "_bg",
    //     lua_ctx
    //         .create_function(move |_, (x, y, z, w): (f32, f32, f32, f32)| {
    //             let mut mutex = &mut switch.write();
    //             mutex.background = vec4(x, y, z, w);
    //             // parking_lot::RwLockWriteGuard::unlock_fair(*mutex);
    //             Ok(1)
    //         })
    //         .unwrap(),
    // ));
    let switch = Arc::clone(&switch_board);
    lua!(
        "cube",
        move |_,
              (name, t, w, n, e, s, b): (
            String,
            String,
            String,
            String,
            String,
            String,
            String
        )| {
            let mut mutex = &mut switch.write();
            mutex.make_queue.push(vec![name, t, b, e, w, s, n]);

            // crate::model::edit_cube(name, [t, e, n, w, s, b]);
            // let mut mutex = &mut switch.write();
            // mutex.tile_queue.push((t, vec4(0., x, y, z)));
            Ok(1)
        },
        "Create a new cube model based on 6 textures"
    );

    let switch = Arc::clone(&switch_board);
    lua!(
        "tile",
        move |_, (t, x, y, z): (String, f32, f32, f32)| {
            // core.world.set_tile(format!("grid"), 0, 0, 16 * 0);
            let mut mutex = &mut switch.write();
            mutex.tile_queue.push((t, vec4(0., x, y, z)));
            Ok(1)
        },
        "Set a tile within 3d space."
    );

    let switch = Arc::clone(&switch_board);
    lua!(
        "tile_done",
        move |_, (): ()| {
            let mutex = &mut switch.write();
            mutex.dirty = true;
            Ok(1)
        },
        "Complete tile creation by triggering a redraw."
    );

    let switch = Arc::clone(&switch_board);
    lua!(
        "tile_quick",
        move |_, (t, x, y, z): (String, f32, f32, f32)| {
            // core.world.set_tile(format!("grid"), 0, 0, 16 * 0);
            let mut mutex = &mut switch.write();
            mutex.tile_queue.push((t, vec4(0., x, y, z)));
            mutex.dirty = true;
            Ok(1)
        },
        "Set a tile within 3d space and immediately trigger a redraw."
    );

    // MARK
    lua!(
        "is_tile",
        move |_, (x, y, z): (f32, f32, f32)| {
            // core.world.set_tile(format!("grid"), 0, 0, 16 * 0);
            // let mut mutex = &mut switch.read();
            // mutex.tile_queue.push((t, vec4(0., x, y, z)));
            // mutex.dirty = true;
            let (tx, rx) = sync_channel::<i32>(0);
            pitcher.send((1, String::new(), x as i32, y as i32, z as i32, tx));
            Ok(match rx.recv() {
                Ok(n) => n == 1,
                Err(e) => {
                    // err(e.to_string());
                    false
                }
            })
        },
        "Set a tile within 3d space and immediately trigger a redraw."
    );

    let switch = Arc::clone(&switch_board);
    lua!(
        "space",
        move |_, (): ()| { Ok(switch.read().space) },
        "Space is down"
    );

    // let pitchy = Arc::new(pitcher);
    lua!(
        "key",
        move |_, (key): (String)| {
            // match key_match(key) {
            //     Some(k) => Ok(crate::controls::input_manager.read().key_held(k)),
            //     None => Ok(false),
            // }
            // let (tx, rx) = sync_channel::<i32>(0);
            // pitcher.send((0, key, tx));
            // Ok(match rx.recv() {
            //     Ok(n) => n == 1,
            //     Err(e) => {
            //         // err(e.to_string());
            //         false
            //     }
            // })
            Ok(false)
        },
        "Check if key is held down"
    );

    lua!(
        "key_pressed",
        |_, (key): (String)| {
            match key_match(key) {
                Some(k) => Ok(crate::controls::input_manager.read().key_pressed(k)),
                None => Ok(false),
            }
        },
        "Check if key is pressed breifly"
    );

    lua!(
        "key_released",
        |_, (key): (String)| {
            match key_match(key) {
                Some(k) => Ok(crate::controls::input_manager.read().key_released(k)),
                None => Ok(false),
            }
        },
        "Check if key is released"
    );

    // let switch = Arc::clone(&switch_board);
    // res(lua_globals.set(
    //     "_prt",
    //     lua_ctx
    //         .create_function(
    //             move |_,
    //                   (tex, n, x, y, z, vx, vy, vz): (
    //                 String,
    //                 f32,
    //                 f32,
    //                 f32,
    //                 f32,
    //                 f32,
    //                 f32,
    //                 f32,
    //             )| {
    //                 let mut mutex = &mut switch.write();
    //                 mutex.dirty = true;
    //                 Ok(1)
    //             },
    //         )
    //         .unwrap(),
    // ));
    // let switch = Arc::clone(&switch_board);

    lua!(
        "spawn",
        move |_, (asset, x, y, z): (String, f64, f64, f64)| {
            // pub fn add(&mut self, x: f32, y: f32, z: f32) -> LuaEnt {
            // let ents = lua.globals().get::<&str, Table>("_ents")?;
            let mut guard = crate::ent_master.write();
            let eman = guard.get_mut().unwrap();
            let index = eman.ent_table.len();

            let ent = crate::lua_ent::LuaEnt::new(index as f64, asset, x, y, z);

            // Rc<RefCell
            let wrapped = Arc::new(Mutex::new(ent));

            let outputEnt = Arc::clone(&wrapped);

            eman.create_from_lua(wrapped);
            // ents.g
            // ents.set(index, ent)?;
            // lua.create_registry_value(t)
            // println!("we made an ent");
            // ents.get
            // lua.
            // let mut e2 = ents.get::<i64, LuaEnt>(index)?;
            // e2.x += 10.;
            // ents.set(index, e2);
            // Ok(ents.get::<i64, LuaEnt>(index)?)
            Ok(outputEnt)
        },
        "Spawn an entity"
    );

    lua!(
        "add",
        move |lua, (e): (LuaEnt)| {
            let ents = lua.globals().get::<&str, Table>("_ents")?;
            ents.set(e.get_id(), e);
            Ok(())
        },
        "Add an entity to our global render table"
    );

    /**
     * // YELLOW
     *  use to store an entity between context, for moving entities between games maybe?
     *  lua.create_registry_value(t)
     */

    lua!(
        "_self_destruct",
        |_, (): ()| {
            Ok(())
            //
        },
        "I guess blow up the lua core?"
    );

    Ok(())
}

fn res(target: &str, r: Result<(), Error>) {
    match r {
        Err(err) => {
            let st = format!("🔴lua::problem setting default lua function {}", target);
            println!("{}", st);
            crate::log::log(st.to_string());
        }
        _ => {}
    }
}

pub fn reset(core: &mut Core) {
    crate::texture::reset();
    crate::model::reset();

    core.lua_master.die();
    core.world.destroy_it_all();

    // TODO why doe sent reset panic?
    // let mut guard = crate::ent_master.write();
    // guard.get_mut().unwrap().reset();
}

pub fn load(core: &mut Core, sub_command: Option<String>) {
    // let mut mutex = crate::lua_master.lock();

    core.lua_master.start(Arc::clone(&core.switch_board));

    crate::texture::reset();

    match sub_command {
        Some(s) => {
            crate::asset::unpack(&core.device, &format!("{}.game.png", s), &core.lua_master);
        }
        None => {
            crate::asset::walk_files(Some(&core.device), &core.lua_master);
        }
    };

    crate::texture::refinalize(&core.device, &core.queue, &core.master_texture);
    // DEV  TODO
    // for e in &mut entity_manager.entities {
    //     e.hot_reload();
    // }
    core.lua_master.call_main();
    log("=================================================".to_string());
    log("loaded into game".to_string());
}

pub fn reload(core: &mut Core) {
    reset(core);
    load(core, None);
}

fn key_match(key: String) -> Option<VirtualKeyCode> {
    Some(match key.to_lowercase().as_str() {
        "a" => VirtualKeyCode::A,
        "b" => VirtualKeyCode::B,
        "c" => VirtualKeyCode::C,
        "d" => VirtualKeyCode::D,
        "e" => VirtualKeyCode::E,
        "f" => VirtualKeyCode::F,
        "g" => VirtualKeyCode::G,
        "h" => VirtualKeyCode::H,
        "i" => VirtualKeyCode::I,
        "j" => VirtualKeyCode::J,
        "k" => VirtualKeyCode::K,
        "l" => VirtualKeyCode::L,
        "m" => VirtualKeyCode::M,
        "n" => VirtualKeyCode::N,
        "o" => VirtualKeyCode::O,
        "p" => VirtualKeyCode::P,
        "q" => VirtualKeyCode::Q,
        "r" => VirtualKeyCode::R,
        "s" => VirtualKeyCode::S,
        "t" => VirtualKeyCode::T,
        "u" => VirtualKeyCode::U,
        "v" => VirtualKeyCode::V,
        "w" => VirtualKeyCode::W,
        "x" => VirtualKeyCode::X,
        "y" => VirtualKeyCode::Y,
        "z" => VirtualKeyCode::Z,
        "space" => VirtualKeyCode::Space,
        "lctrl" => VirtualKeyCode::LControl,
        "rctrl" => VirtualKeyCode::RControl,
        _ => return None,
    })
}

fn log(str: String) {
    println!("com::{}", str);
    crate::log::log(format!("com::{}", str));
}

fn err(str: String) {
    println!("com_err::{}", str);
    crate::log::log(format!("com_err::{}", str));
}

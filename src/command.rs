use crate::{
    lua_define::MainPacket,
    lua_ent::LuaEnt,
    pad::Pad,
    sound::SoundPacket,
    switch_board::SwitchBoard,
    world::{TileCommand, TileResponse, World},
    Core,
};

use glam::vec4;
use itertools::Itertools;
use mlua::{Error, Lua, Table};
use parking_lot::{Mutex, RwLock};
use std::{
    path::Path,
    rc::Rc,
    sync::{
        mpsc::{Sender, SyncSender},
        Arc,
    },
};

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
                if segments.len() > 2 {
                    Some(segments[2].to_string())
                } else {
                    None
                },
                if segments.len() > 3 {
                    Some(segments[3].to_string())
                } else {
                    None
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
            if segments.len() > 1 {
                let name = segments[1].to_string();
                crate::zip_pal::unpack_and_save(
                    crate::zip_pal::get_file_buffer(&format!("{}.game.zip", name)),
                    &format!("{}.zip", name),
                );
            } else {
                log("$unpack <file without .game.png>".to_string());
            }
        }
        "$load" => {
            load(
                core,
                if segments.len() > 1 {
                    // let targ = format!("{}.game.png", segments[1].to_string());
                    // let file = crate::zip_pal::get_file_buffer(&targ);
                    Some(segments[1].to_string())
                } else {
                    None
                },
                None,
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
        "$new" => {
            if segments.len() > 1 {
                let name = segments[1].to_string();
                crate::asset::make_directory(name.clone());
                log(format!("created directory {}", name));
            } else {
                log("$new <name>".to_string());
            }
        }
        &_ => return false,
    }
    true
}

pub fn init_lua_sys(
    lua_ctx: &Lua,
    lua_globals: &Table,
    switch_board: Arc<RwLock<SwitchBoard>>,
    main_pitcher: Sender<MainPacket>,
    world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
    singer: Sender<SoundPacket>,
    bits: Rc<Mutex<[bool; 256]>>,
    gamepad: Rc<Mutex<Pad>>,
) -> Result<(), Error> {
    println!("init lua sys");

    let default_func = lua_ctx
        .create_function(|_, _: f32| Ok("placeholder func uwu"))
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
        move |_, n: f64| {
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
        move |_, (x, y, z, w): (f32, f32, f32, Option<f32>)| {
            let mutex = &mut switch.write();
            mutex.background = vec4(
                x,
                y,
                z,
                match w {
                    Some(w) => w,
                    None => 1.,
                },
            );
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
    // let sender = world_sender.clone();
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
            // MARK the make command
            let mutex = &mut switch.write();
            // World::make(&sender, name, t, b, e, w, s, n);
            mutex.make_queue.push(vec![name, t, b, e, w, s, n]);
            mutex.dirty = true;
            drop(mutex);

            // while (match switch.try_read() {
            //     Some(r) => r.dirty,
            //     None => true,
            // }) {
            //     // println!("waiting for make_queue to empty");
            //     // std::thread::sleep(std::time::Duration::from_millis(10));
            // }
            // println!("MAKE {:?}", mutex.make_queue);
            // crate::model::edit_cube(name, [t, e, n, w, s, b]);
            // let mut mutex = &mut switch.write();
            // mutex.tile_queue.push((t, vec4(0., x, y, z)));
            Ok(1)
        },
        "Create a new cube model based on 6 textures"
    );

    let sender = world_sender.clone();
    lua!(
        "tile",
        move |_, (t, x, y, z, r): (String, i32, i32, i32, Option<u8>)| {
            // core.world.set_tile(format!("grid"), 0, 0, 16 * 0);
            // let mut mutex = &mut switch.write();
            // mutex.tile_queue.push((t, vec4(0., x, y, z)));
            let ro = match r {
                Some(i) => i,
                None => 0,
            };

            World::set_tile(&sender, t, x, y, z, ro);
            Ok(1)
        },
        "Set a tile within 3d space."
    );

    let sender = world_sender.clone();
    lua!(
        "drop_chunk",
        move |_, (x, y, z): (i32, i32, i32)| {
            // let mutex = &mut switch.write();
            // mutex.dirty = true;
            World::drop_chunk(&sender, x, y, z);
            Ok(1)
        },
        "Crude deletion of a 16x16x16 chunk. Extremely efficient for large area tile changes"
    );

    let sender = world_sender.clone();
    lua!(
        "clear_tiles",
        move |_, (_x, _y, _z): (i32, i32, i32)| {
            // let mutex = &mut switch.write();
            // mutex.dirty = true;
            World::clear_tiles(&sender);
            Ok(1)
        },
        "Crude deletion of a 16x16x16 chunk. Extremely efficient for large area tile changes"
    );

    // let switch = Arc::clone(&switch_board);
    // lua!(
    //     "tile_quick",
    //     move |_, (t, x, y, z): (String, f32, f32, f32)| {
    //         // core.world.set_tile(format!("grid"), 0, 0, 16 * 0);
    //         let mut mutex = &mut switch.write();
    //         mutex.tile_queue.push((t, vec4(0., x, y, z)));
    //         mutex.dirty = true;
    //         Ok(1)
    //     },
    //     "Set a tile within 3d space and immediately trigger a redraw."
    // );

    // MARK
    // BLUE TODO this function is expensive? if called twice in one cycle it ruins key press checks??
    let sender = world_sender.clone();
    lua!(
        "is_tile",
        move |_, (x, y, z): (i32, i32, i32)| {
            // core.world.set_tile(format!("grid"), 0, 0, 16 * 0);
            // let mut mutex = &mut switch.read();
            // mutex.tile_queue.push((t, vec4(0., x, y, z)));
            // mutex.dirty = true;

            // let (tx, rx) = sync_channel::<i32>(0);
            // pitcher.send((1, String::new(), x as i32, y as i32, z as i32, tx));
            // Ok(match rx.recv() {
            //     Ok(n) => n == 1,
            //     Err(e) => {
            //         false
            //     }
            // })
            Ok(World::is_tile(&sender, x, y, z))
        },
        "Set a tile within 3d space and immediately trigger a redraw."
    );

    lua!(
        "anim",
        move |_, (name, items, speed): (String, Vec<String>, Option<f64>)| {
            // println!("we have anims {:?}", items);
            let anim_speed = match speed {
                Some(s) => s as u32,
                None => 16,
            };
            crate::texture::ANIMATIONS.write().insert(
                name,
                (
                    items
                        .iter()
                        .map(|i| crate::texture::get_tex(i))
                        .collect_vec(),
                    anim_speed,
                ),
            );
            Ok(true)
        },
        "Set an animation"
    );

    // let pitchy = Arc::new(pitcher);
    lua!(
        "key",
        move |_, key: String| {
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
            // let c = key.to_lowercase().chars().collect::<Vec<char>>()[0] as usize;

            // if bits.lock()[key_match(key.clone())] {

            // }
            Ok(bits.lock()[key_match(key)])
            // match bits.lock() {
            //     Ok(b) => {
            //         let z = b;
            //         Ok(b[c])
            //     }
            //     _ => Ok(false),
            // }
        },
        "Check if key is held down"
    );

    let gam = Rc::clone(&gamepad);
    lua!(
        "button",
        move |_, button: String| { Ok(gam.lock().check(button) != 0.) },
        "Check if button is held down"
    );

    lua!(
        "analog",
        move |_, button: String| { Ok(gamepad.lock().check(button)) },
        "Check how much a button is pressed, axis gives value between -1 and 1"
    );

    // res(
    //     "t",
    //     lua_globals.set(
    //         "_bg",
    //         lua_ctx
    //             .create_function(move |_, (key): (String)| {
    //                 // let h = match *gamepad.lock() {
    //                 //     Some(game) => game.is_pressed(Button::South),
    //                 //     _ => false,
    //                 // };
    //                 let b2 = b.lock();
    //                 Ok(1)
    //             })
    //             .unwrap(),
    //     ),
    // );

    // lua!(
    //     "key_pressed",
    //     |_, (key): (String)| {
    //         match key_match(key) {
    //             Some(k) => Ok(crate::controls::input_manager.read().key_pressed(k)),
    //             None => Ok(false),
    //         }
    //     },
    //     "Check if key is pressed breifly"
    // );

    // lua!(
    //     "key_released",
    //     |_, (key): (String)| {
    //         match key_match(key) {
    //             Some(k) => Ok(crate::controls::input_manager.read().key_released(k)),
    //             None => Ok(false),
    //         }
    //     },
    //     "Check if key is released"
    // );

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
            let mut guard = crate::ent_master.write();
            let eman = guard.get_mut().unwrap();
            let index = eman.ent_table.len();

            let ent = crate::lua_ent::LuaEnt::new(index as f64, asset, x, y, z);

            // Rc<RefCell
            let wrapped = Arc::new(std::sync::Mutex::new(ent));

            let output_ent = Arc::clone(&wrapped);

            eman.create_from_lua(wrapped);

            Ok(output_ent)
        },
        "Spawn an entity"
    );

    lua!(
        "add",
        move |lua, e: LuaEnt| {
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
    let switch = Arc::clone(&switch_board);
    lua!(
        "crt",
        move |_, table: Table| {
            for it in table.pairs() {
                match it {
                    Ok(pair) => {
                        switch
                            .write()
                            .remaps
                            .push(("globals".to_string(), pair.0, pair.1));
                    }
                    _ => {}
                }
            }
            switch.write().dirty = true;
            Ok(())
        },
        "Set the CRT parameters"
    );

    // let switch = Arc::clone(&switch_board);
    let pitcher = main_pitcher.clone();
    lua!(
        "campos",
        move |_, (x, y, z): (f32, f32, f32)| {
            // let (tx, rx) = sync_channel::<bool>(0);
            // println!("🧲 eyup send pos");
            pitcher.send((MainCommmand::CamPos, x, y, z, 0.));
            // Ok(match rx.recv() {
            //     Ok(v) => (true),
            //     _ => (false),
            // })
            Ok(())
        },
        "Set the camera position"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "camrot",
        move |_, (x, y): (f32, f32)| {
            // let (tx, rx) = sync_channel::<bool>(0);

            pitcher.send((MainCommmand::CamRot, x, y, 0., 0.));
            // sender.send((TileCommand::Is(ivec3(x, y, z)), tx));
            // println!("🧲 eyup send rot");

            // Ok(match rx.recv() {
            //     Ok(v) => (true),
            //     _ => (false),
            // })
            Ok(())
        },
        "Set the camera rotation by azimuth and elevation"
    );
    let sing = singer.clone();
    lua!(
        "sound",
        move |_, (freq, length): (f32, Option<f32>)| {
            let len = match length {
                Some(l) => l,
                None => 1.,
            };

            println!("freq {}", freq);

            sing.send((freq, len, vec![], vec![]));
            Ok(())
        },
        "Make sound"
    );

    lua!(
        "instr",
        move |_, (length, notes, amps): (f32, Vec<f32>, Option<Vec<f32>>)| {
            println!("freqs {:?}", notes);

            singer.send((
                -2.,
                length,
                notes,
                match amps {
                    Some(a) => a,
                    None => vec![],
                },
            ));
            Ok(())
        },
        "Make sound"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "sqr",
        move |_, (x, y, w, h): (f32, f32, f32, f32)| {
            pitcher.send((MainCommmand::Square, x, y, w, h));
            Ok(())
        },
        "Draw a square on the gui"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "line",
        move |_, (x, y, x2, y2): (f32, f32, f32, f32)| {
            pitcher.send((MainCommmand::Line, x, y, x2, y2));
            Ok(())
        },
        "Draw a line on the gui"
    );

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
            log(format!(
                "🔴lua::problem setting default lua function {}, {}",
                target, err
            ));
        }
        _ => {}
    }
}

pub fn reset(core: &mut Core) {
    crate::texture::reset();
    crate::model::reset();
    core.gui.clean();

    core.lua_master.die();
    core.world.destroy_it_all();
    core.global.iteration = 0;

    // TODO why doe sent reset panic?
    let mut guard = crate::ent_master.write();
    guard.get_mut().unwrap().reset();
}

pub fn load_from_string(core: &mut Core, sub_command: Option<String>) {
    // let sub = match sub_command {
    //     Some(s) => {
    //         // log(format!("load from string {}", s));
    //         Some((
    //             s.clone(),
    //             crate::zip_pal::get_file_buffer(&format!("{}.game.png", s)),
    //         ))
    //     }
    //     None => None,
    // };
    load(core, sub_command, None);
}

/**
 * Load a game from a zip file, directory, or included bytes
 * @param core
 * @param game_path: path to either a directory of game files or a single game file
 * @param payload: included bytes, only used as part of build process
 */
pub fn load(core: &mut Core, game_path: Option<String>, payload: Option<Vec<u8>>) {
    // let mut mutex = crate::lua_master.lock();
    let catcher = core.lua_master.start(
        Arc::clone(&core.switch_board),
        core.world.sender.clone(),
        core.singer.clone(),
    );

    core.catcher = Some(catcher);
    crate::texture::reset();

    // if we get a path and it's a file, it needs to be unpacked, if it's a custom directoty we walk it, otherwise walk the local directory
    match game_path {
        Some(s) => match payload {
            Some(p) => {
                crate::asset::unpack(&core.device, &s, p, &core.lua_master);
            }
            None => {
                let mut path = crate::asset::determine_path(Some(s.clone()));
                core.global.loaded_directory = Some(s.clone());
                if path.is_dir() {
                    crate::asset::walk_files(Some(&core.device), &core.lua_master, path);
                } else {
                    match path.file_name() {
                        Some(file_name) => {
                            let new_path = format!("{:?}.game.png", file_name);
                            drop(file_name);
                            path.set_file_name(new_path);
                            if path.is_file() {
                                let buff = crate::zip_pal::get_file_buffer_from_path(path);
                                crate::asset::unpack(&core.device, &s, buff, &core.lua_master);
                            } else {
                                err(format!("{} is not a file or directory", s));
                            }
                        }
                        None => {
                            err(format!("{} is not a file or directory", s));
                        }
                    };
                }
            }
        },
        None => {
            let path = crate::asset::determine_path(None);
            crate::asset::walk_files(Some(&core.device), &core.lua_master, path);
        }
    };

    crate::texture::refinalize(&core.queue, &core.master_texture);
    // DEV  TODO
    // for e in &mut entity_manager.entities {
    //     e.hot_reload();
    // }
    let dir = match &core.global.loaded_directory {
        Some(s) => s.clone(),
        None => "_".to_string(),
    };
    log("=================================================".to_string());
    log(format!("loaded into game {}", dir));
    log("-------------------------------------------------".to_string());
    core.update();
    core.lua_master.call_main();
}

pub fn reload(core: &mut Core) {
    reset(core);
    #[cfg(feature = "include_auto")]
    {
        log("auto loading included bytes".to_string());
        let payload = include_bytes!("../auto.game.png").to_vec();
        load(core, Some("INCLUDE_AUTO".to_string()), Some(payload));
    }
    #[cfg(not(feature = "include_auto"))]
    {
        load(
            core,
            match core.global.loaded_directory {
                Some(ref s) => Some(s.clone()),
                None => None,
            },
            None,
        );
    }
}

fn key_match(key: String) -> usize {
    match key.to_lowercase().as_str() {
        "1" => 0,
        "2" => 1,
        "3" => 2,
        "4" => 3,
        "5" => 4,
        "6" => 5,
        "7" => 6,
        "8" => 7,
        "9" => 8,
        "0" => 9,
        "a" => 10,
        "b" => 11,
        "c" => 12,
        "d" => 13,
        "e" => 14,
        "f" => 15,
        "g" => 16,
        "h" => 17,
        "i" => 18,
        "j" => 19,
        "k" => 20,
        "l" => 21,
        "m" => 22,
        "n" => 23,
        "o" => 24,
        "p" => 25,
        "q" => 26,
        "r" => 27,
        "s" => 28,
        "t" => 29,
        "u" => 30,
        "v" => 31,
        "w" => 32,
        "x" => 33,
        "y" => 34,
        "z" => 35,
        "escape" => 35,
        "f1" => 36,
        "f2" => 37,
        "f3" => 38,
        "f4" => 39,
        "f5" => 40,
        "f6" => 41,
        "f7" => 42,
        "f8" => 43,
        "f9" => 44,
        "f10" => 45,
        "f11" => 46,
        "f12" => 47,
        "f13" => 48,
        "f14" => 49,
        "f15" => 50,
        "snapshot" => 51,
        "delete" => 56,
        "left" => 70,
        "up" => 71,
        "right" => 72,
        "down" => 73,
        "back" => 64,
        "return" => 65,
        // "space" => {
        //     println!("space");
        //     return 66;
        // }
        "space" => 76,

        // "space" => VirtualKeyCode::Space,
        // "lctrl" => VirtualKeyCode::LControl,
        // "rctrl" => VirtualKeyCode::RControl,
        _ => 255,
    }
}

fn log(str: String) {
    println!("com::{}", str);
    crate::log::log(format!("com::{}", str));
}

fn err(str: String) {
    println!("com_err::{}", str);
    crate::log::log(format!("com_err::{}", str));
}

pub enum MainCommmand {
    Line,
    Square,
    CamPos,
    CamRot,
}

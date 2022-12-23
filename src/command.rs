use crate::{
    bundle::BundleResources,
    gui::GuiMorsel,
    lg,
    lua_define::MainPacket,
    lua_ent::LuaEnt,
    lua_img::LuaImg,
    pad::Pad,
    sound::{Instrument, Note, SoundCommand},
    tile::Chunk,
    types::ValueMap,
    world::{TileCommand, TileResponse, World},
    Core,
};

#[cfg(feature = "online_capable")]
use crate::online::MovePacket;

use glam::{vec4, Vec4};
use image::RgbaImage;
use itertools::Itertools;
use mlua::{
    AnyUserData, Error, Lua, Table, UserData,
    Value::{self, Nil},
};
use parking_lot::Mutex;

use std::{
    cell::RefCell,
    path::Path,
    rc::Rc,
    sync::{
        mpsc::{Sender, SyncSender},
        Arc,
    },
};

/** Private commands not reachable by lua code, but also works without lua being loaded */
pub fn init_con_sys(core: &mut Core, s: &str) -> bool {
    let bundle_id = core.bundle_manager.console_bundle_target;
    let lua = core.bundle_manager.get_lua();
    if s.len() <= 0 {
        return false;
    }
    let segments = s.trim().split(" ").collect::<Vec<&str>>();

    match segments[0] {
        "die" => {
            // this chunk could probably be passed directly to lua core but being it's significance it felt important to pass into our pre-system check for commands
            lua.die();
        }
        "bundles" => {
            crate::lg!("{}", core.bundle_manager.list_bundles());
        }
        "pack" => {
            // name, path, cartridge pic
            crate::asset::pack(
                &mut core.tex_manager,
                &mut core.model_manager,
                &mut core.world,
                bundle_id,
                lua,
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
                core.global.debug,
            );
        }
        "superpack" => {
            log(crate::asset::super_pack(&if segments.len() > 1 {
                format!("{}", segments[1])
            } else {
                "game".to_string()
            })
            .to_string());
        }
        "unpack" => {
            if segments.len() > 1 {
                let name = segments[1].to_string();
                crate::zip_pal::unpack_and_save(
                    crate::zip_pal::get_file_buffer(&format!("./{}.game.zip", name)),
                    &format!("{}.zip", name),
                );
            } else {
                log("unpack <file without .game.png>".to_string());
            }
        }
        "load" => {
            hard_reset(core);
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
                None,
                None,
            );
        }
        "reset" => hard_reset(core),
        "reload" => reload(core, bundle_id),
        "atlas" => {
            core.tex_manager.save_atlas();
        }
        "dev" => {
            core.global.debug = !core.global.debug;
            log(format!("debug is {}", core.global.debug));
        }
        "ls" => {
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
        "ugh" => {
            // TODO ugh?
        }
        "clear" => crate::log::clear(),
        "test" => {
            log("that test worked, yipee".to_string());
        }
        "new" => {
            if segments.len() > 1 {
                let name = segments[1].to_string();
                crate::asset::make_directory(name.clone());
                log(format!("created directory {}", name));
            } else {
                log("new <name>".to_string());
            }
        }
        "find" => {
            if segments.len() > 2 {
                match segments[1] {
                    "model" => {
                        let v = core
                            .model_manager
                            .search_model(&segments[2].to_string(), None);
                        if v.len() > 0 {
                            log(format!("models -> {}", v.join(",")));
                        } else {
                            log("no models".to_string());
                        }
                    }
                    _ => {
                        log("???".to_string());
                    }
                }
            } else {
                log("find <model | ???> <search-query>".to_string());
            }
        }
        "parse" => {
            if segments.len() > 1 {
                let s = segments[1].to_string();
                crate::parse::test(&s)
            }
        }
        &_ => return false,
    }
    true
}

#[cfg(feature = "online_capable")]
type OnlineType = Option<(Sender<MovePacket>, Receiver<MovePacket>)>;
#[cfg(not(feature = "online_capable"))]
type OnlineType = Option<bool>;

pub fn init_lua_sys(
    lua_ctx: &Lua,
    lua_globals: &Table,
    bundle_id: u8,
    // switch_board: Arc<RwLock<SwitchBoard>>,
    main_pitcher: Sender<MainPacket>,
    world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
    gui_in: Rc<RefCell<GuiMorsel>>,
    net_sender: OnlineType,
    singer: Sender<SoundCommand>,
    keys: Rc<RefCell<[bool; 256]>>,
    diff_keys: Rc<RefCell<[bool; 256]>>,
    mice: Rc<RefCell<[f32; 8]>>,
    gamepad: Rc<RefCell<Pad>>,
    ent_counter: Rc<Mutex<u64>>,
    // ent_tracker: Rc<RefCell<FxHashMap<u64,bool>
) -> Result<(), Error>
// where N: 
// #[cfg(feature = "online_capable")]
// Option<(Sender<MovePacket>, Receiver<MovePacket>)> 
// #[cfg(not(feature = "online_capable"))]
// Option<bool>
{
    println!("init lua sys");

    #[cfg(feature = "online_capable")]
    {
        let (netout, netin) = match net_sender {
            Some((nout, nin)) => (Some(nout), Some(nin)),
            _ => (None, None),
        };
    }

    let default_func = lua_ctx
        .create_function(|_, _: f32| Ok("placeholder func uwu"))
        .unwrap();
    res(
        "_default_func",
        lua_globals.set("_default_func", default_func),
    );

    let mut command_map: Vec<(String, String)> = vec![];

    // lua_globals.set("_ents", lua_ctx.create_table()?);
    lua_globals.set("pi", std::f64::consts::PI);
    lua_globals.set("tau", std::f64::consts::PI * 2.0);

    // lua_ctx.set_warning_function(|a, b, f| {
    //     log(format!("hi {:?}", b));
    //     Ok(())
    // });

    #[macro_export]
    macro_rules! lua {
        ($name:expr,$closure:expr,$desc:expr) => {
            command_map.push(($name.to_string(), $desc.to_string()));
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

    // lua!(
    //     "push",
    //     move |_, n: f64| {
    //         // let ents = lua.globals().get::<&str, Table>("_ents")?;
    //         // ents.macro_export

    //         let mut guard = crate::ent_master.write();
    //         let eman = guard.get_mut().unwrap();

    //         let ents = &eman.ent_table;
    //         for wrapped_ent in &mut ents.iter() {
    //             let mut eg = wrapped_ent.lock().unwrap();
    //             eg.x += n;
    //         }

    //         Ok(())
    //     },
    //     "Pushes entities"
    // );

    // let switch = Arc::clone(&switch_board);

    let pitcher = main_pitcher.clone();
    lua!(
        "cube",
        move |_,
              (name, t, w, n, e, s, b): (
            String,
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>
        )| {
            // let mutex = &mut switch.write();
            let (tx, rx) = std::sync::mpsc::sync_channel::<u8>(0);
            // println!("this far-1");

            pitcher.send((
                bundle_id,
                MainCommmand::Make(
                    vec![
                        name,
                        t.clone(),
                        b.unwrap_or(t.clone()),
                        e.unwrap_or(t.clone()),
                        w.unwrap_or(t.clone()),
                        s.unwrap_or(t.clone()),
                        n.unwrap_or(t),
                    ],
                    tx,
                ),
            ));
            // match rx.recv() {
            //     Ok(_) => {}
            //     Err(_) => {}
            // }
            // mutex.make_queue.push(vec![name, t, b, e, w, s, n]);
            // mutex.dirty = true;
            // drop(mutex);

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
        move |_, (): ()| {
            // let mutex = &mut switch.write();
            // mutex.dirty = true;
            World::clear_tiles(&sender);
            Ok(1)
        },
        "Crude deletion of a 16x16x16 chunk. Extremely efficient for large area tile changes"
    );

    // MARK
    // BLUE TODO this function is expensive? if called twice in one cycle it ruins key press checks??
    let sender = world_sender.clone();
    lua!(
        "is_tile",
        move |_, (x, y, z): (i32, i32, i32)| { Ok(World::is_tile(&sender, x, y, z)) },
        "Set a tile within 3d space and immediately trigger a redraw."
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "anim",
        move |_, (name, items, speed): (String, Vec<String>, Option<f64>)| {
            // println!("we have anims {:?}", items);
            let anim_speed = match speed {
                Some(s) => s as u32,
                None => 16,
            };
            pitcher.send((bundle_id, MainCommmand::Anim(name, items, anim_speed)));
            Ok(true)
        },
        "Set an animation"
    );

    // let pitchy = Arc::new(pitcher);
    let dkeys = diff_keys.clone();
    lua!(
        "key",
        move |_, (key, volatile): (String, Option<bool>)| {
            match volatile {
                Some(true) => Ok(dkeys.borrow()[key_match(key)]),
                _ => Ok(keys.borrow()[key_match(key)]),
            }
        },
        "Check if key is held down"
    );

    lua!(
        "input",
        move |_, _: ()| {
            let h: String = diff_keys
                .borrow()
                .iter()
                .enumerate()
                .filter_map(|(i, k)| if *k { key_unmatch(i) } else { None })
                .collect();

            // .filter(|(k, v)| **v)
            // .map(|(k, v)| char::from_u32((87 + k) as u32).unwrap())
            // .join("");
            Ok(h)
        },
        "Check if key is held down"
    );

    lua!(
        "mouse",
        move |lu, (): ()| {
            let t = lu.create_table()?;
            let m = mice.borrow();
            t.set("x", m[0])?;
            t.set("y", m[1])?;
            t.set("dx", m[2])?;
            t.set("dy", m[3])?;
            // t.set("z",m[2])?;
            t.set("m1", m[4] > 0.)?;
            t.set("m2", m[5] > 0.)?;
            t.set("m3", m[6] > 0.)?;

            Ok(t)
        },
        " Get mouse position from 0.-1."
    );

    let gam = Rc::clone(&gamepad);
    lua!(
        "button",
        move |_, button: String| { Ok(gam.borrow().check(button) != 0.) },
        "Check if button is held down"
    );

    lua!(
        "analog",
        move |_, button: String| { Ok(gamepad.borrow().check(button)) },
        "Check how much a button is pressed, axis gives value between -1 and 1"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "spawn",
        move |lua, (asset, x, y, z, s): (String, f64, f64, f64, Option<f64>)| {
            // let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<Arc<std::sync::Mutex<LuaEnt>>>>(0);
            let id = *ent_counter.lock();
            *ent_counter.lock() += 1;

            let ent = crate::lua_ent::LuaEnt::new(id, asset, x, y, z, s.unwrap_or(1.));
            let wrapped = Arc::new(std::sync::Mutex::new(ent));

            // match pitcher.send(MainCommmand::Spawn(asset, x, y, z, s.unwrap_or(1.), 1, tx)) {

            match pitcher.send((bundle_id, MainCommmand::Spawn(Arc::clone(&wrapped)))) {
                Ok(_) => {}
                Err(er) => log(format!("error sending spawn {}", er)),
            }

            // Ok(match rx.recv() {
            //     Ok(mut e) => e.remove(0),
            //     Err(e) => Arc::new(std::sync::Mutex::new(LuaEnt::empty())),
            // })
            Ok(wrapped)
        },
        "Spawn an entity"
    );
    let pitcher = main_pitcher.clone();
    lua!(
        "group",
        move |_, (parent,child ): (Arc<std::sync::Mutex<LuaEnt>>,Arc<std::sync::Mutex<LuaEnt>>)| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<bool>(0);
            let parentId=parent.lock().unwrap().get_id();
            let childId=child.lock().unwrap().get_id();
            match pitcher.send((bundle_id,MainCommmand::Group(parentId,childId, tx))) {
                Ok(_) => {}
                Err(er) => log(format!("error sending spawn {}", er)),
            };
            match rx.recv(){
                Ok(_) => {},
                Err(_) => {}
            };

            Ok(())
        },
        "Groups an entity onto another entity"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "kill",
        move |lu, (ent): (Value)| {
            //Arc<std::sync::Mutex<LuaEnt>>
            let id = match ent {
                Value::UserData(g) => {
                    // let gg = g.borrow().unwrap();

                    // match g.borrow()

                    // .downcast_ref::<LuaEnt>() {
                    //     Ok(ll) => {}
                    //     _ => {}
                    // }

                    // // g.into()
                    // let en:Arc<std::sync::Mutex<LuaEnt>> = g.into();
                    // g.take()

                    // match g.get_named_user_value::<_, u64>("id") {
                    //     Ok(g) => g,
                    //     _ => 0,
                    // }
                    // println!("userdata is {}", g.is::<Arc<std::sync::Mutex<LuaEnt>>>());
                    match g.borrow::<Arc<std::sync::Mutex<LuaEnt>>>() {
                        Ok(r) => {
                            // println!("internal");
                            r.lock().unwrap().get_id()
                        }
                        Err(_) => 0,
                    }

                    // g.borrow_mut()
                    //     .downcast_mut::<Arc<std::sync::Mutex<LuaEnt>>>()
                    //     .unwrap();
                }
                Value::Integer(n) => n as u64,
                Value::Number(n) => n as u64,
                _ => 0,
            };
            // println!("ent id {}", id);
            match pitcher.send((bundle_id, MainCommmand::Kill(id))) {
                Ok(_) => {}
                Err(er) => log(format!("error sending spawn {}", er)),
            }

            // let wrapped = Arc::new(std::sync::Mutex::new(ent));

            Ok(())
        },
        "Kills an entity"
    );
    let pitcher = main_pitcher.clone();
    lua!(
        "reload",
        move |_, (): ()| {
            // println!("hit reset");
            match pitcher.send((bundle_id, MainCommmand::Reload())) {
                Ok(_) => {}
                Err(er) => {}
            }
            Ok(())
        },
        "Reset lua context"
    );

    /**
     * // YELLOW
     *  use to store an entity between context, for moving entities between games maybe?
     *  lua.create_registry_value(t)
     */
    // let switch = Arc::clone(&switch_board);
    let pitcher = main_pitcher.clone();
    lua!(
        "attr",
        move |_, table: Table| {
            // pitcher.send(MainCommmand::Globals(table));

            let hash = table_hasher(table);
            // println!("crt {:?}", hash);
            pitcher.send((bundle_id, MainCommmand::Globals(hash)));

            // switch.write().dirty = true;
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
            pitcher.send((bundle_id, MainCommmand::CamPos(glam::vec3(x, y, z))));
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

            pitcher.send((bundle_id, MainCommmand::CamRot(glam::vec2(x, y))));
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

            // println!("freq {}", freq);
            sing.send(SoundCommand::PlayNote(Note::new(0, freq, len, 1.), None));
            // sing.send((freq, len, vec![], vec![]));
            Ok(())
        },
        "Make sound"
    );
    let sing = singer.clone();
    lua!(
        "song",
        move |_, (notes): (Vec<Value>)| {
            // println!("sent chain {}", converted.len());
            let converted = notes
                .iter()
                .filter_map(|v| {
                    // to vector
                    match v {
                        Value::Table(t) => {
                            // let mut vec = Vec::new();

                            // for it in t.pairs() {
                            //     match it {
                            //         Ok(pair) => {
                            //             vec.push(pair.1);
                            //         }
                            //         _ => {}
                            //     }
                            // }

                            if t.raw_len() > 0 {
                                Some(Note::new(
                                    0,
                                    t.get::<usize, f32>(1).unwrap_or(440.),
                                    t.get::<usize, f32>(2).unwrap_or(1.),
                                    1.,
                                ))
                            } else {
                                None
                            }
                        }
                        Value::Number(n) => Some(Note::new(0, *n as f32, 1., 1.)),
                        _ => None,
                    }
                })
                .collect::<Vec<Note>>();

            // for n in converted.iter() {
            //     println!("note {} {}", n.frequency, n.duration);
            // }

            println!("sent chain {}", converted.len());
            sing.send(SoundCommand::Chain(converted, None));
            // sing.send((freq, len, vec![], vec![]));
            Ok(())
        },
        "Make song"
    );

    let sing = singer.clone();
    lua!(
        "silence",
        move |_, (channel): (Option<usize>)| {
            // println!("freqs {:?}", notes);

            sing.send(SoundCommand::Stop(channel.unwrap_or((0))));

            Ok(())
        },
        "Stop sounds on channel"
    );

    lua!(
        "instr",
        move |_, (notes, half): (Vec<f32>, Option<bool>)| {
            // println!("freqs {:?}", notes);

            singer.send(SoundCommand::MakeInstrument(Instrument::new(
                0,
                notes,
                match half {
                    Some(h) => h,
                    None => false,
                },
            )));

            Ok(())
        },
        "Make sound"
    );

    lua!(
        "bg",
        move |_, (x, y, z, w): (mlua::Value, Option<f32>, Option<f32>, Option<f32>)| { Ok(1) },
        ""
    );

    // let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "fill",
        move |_, (r, g, b, a): (mlua::Value, Option<f32>, Option<f32>, Option<f32>)| {
            // pitcher.send((bundle_id, MainCommmand::Fill(get_color(r, g, b, a))));
            let c = get_color(r, g, b, a);
            println!("fill");
            gui.borrow_mut().fill(c.x, c.y, c.z, c.w);
            Ok(1)
        },
        "Set background color"
    );

    let gui = gui_in.clone();
    lua!(
        "pixel",
        move |_,
              (x, y, r, g, b, a): (
            u32,
            u32,
            mlua::Value,
            Option<f32>,
            Option<f32>,
            Option<f32>
        )| {
            let c = get_color(r, g, b, a);
            gui.borrow_mut().pixel(x, y, c.x, c.y, c.z, c.w);
            // pitcher.send((bundle_id, MainCommmand::Pixel(x, y, get_color(r, g, b, a))));
            Ok(1)
        },
        "Set color of pixel at x,y"
    );

    // let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "sky",
        move |_, (): ()| {
            // pitcher.send((bundle_id, MainCommmand::Sky()));
            gui.borrow_mut().target_sky();
            Ok(())
        },
        "Set skybox as draw target"
    );
    // let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "gui",
        move |_, (): ()| {
            // pitcher.send((bundle_id, MainCommmand::Gui()));
            gui.borrow_mut().target_gui();
            Ok(())
        },
        "Set gui as draw target"
    );

    // let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "rect",
        move |_,
              (x, y, w, h, r, g, b, a): (
            Value,
            Value,
            Value,
            Value,
            Value,
            Option<f32>,
            Option<f32>,
            Option<f32>
        )| {
            // pitcher.send((
            //     bundle_id,
            //     MainCommmand::Square(numm(x), numm(y), numm(w), numm(h)),
            // ));
            let c = get_color(r, g, b, a);
            gui.borrow_mut().rect(numm(x), numm(y), numm(w), numm(h), c);
            Ok(())
        },
        "Draw a rectangle on the gui"
    );

    // let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "line",
        move |_, (x, y, x2, y2): (Value, Value, Value, Value)| {
            // pitcher.send((
            //     bundle_id,
            //     MainCommmand::Line(numm(x), numm(y), numm(x2), numm(y2)),
            // ));
            gui.borrow_mut().line(numm(x), numm(y), numm(x2), numm(y2));

            Ok(())
        },
        "Draw a line on the gui"
    );
    // let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "text",
        move |_, (txt, x, y): (String, Option<Value>, Option<Value>)| {
            gui.borrow_mut().text(
                &txt,
                match x {
                    Some(o) => numm(o),
                    _ => (false, 0.),
                },
                match y {
                    Some(o) => numm(o),
                    _ => (false, 0.),
                },
            );

            Ok(())
        },
        "Draw text on the gui at position"
    );
    // let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "dimg",
        move |_, (im, x, y): (AnyUserData, Option<Value>, Option<Value>)| {
                            // println!("got image {}x{} w len {}", w, h, len);
            // if let Value::UserData(imm) = im {

            if let Ok(limg) = im.borrow::<LuaImg>() {
                            gui.borrow_mut().draw_image(
                    &limg.image,
                                match x {
                                    Some(o) => numm(o),
                                    _ => (false, 0.),
                                },
                                match y {
                                    Some(o) => numm(o),
                                    _ => (false, 0.),
                                },
                            );
            };

            // match im {
            //     mlua::Value::UserData(imm) => {
            //         if let Ok(limg) = imm.borrow::<LuaImg>() {
            //             gui.borrow_mut().draw_image(
            //                 &limg.image,
            //                 match x {
            //                     Some(o) => numm(o),
            //                     _ => (false, 0.),
            //                 },
            //                 match y {
            //                     Some(o) => numm(o),
            //                     _ => (false, 0.),
            //                 },
            //             );
            //         };
            //     }
            //     // TODO should we shortcut calling by string?
            //     // mlua::Value::String(s) => {
            //     //     lu.call_function::<_, ()>("gimg", (s,))?;
            //     // }
            //     _ => {}
            // }

            // };

            // if let Ok(img) = im.get::<_, Vec<u8>>("data") {
            // v    if let Ok(w) = im.get::<_, u32>("w") {
            //         if let Ok(h) = im.get::<_, u32>("h") {
            //             let len = img.len();
            //             if let Some(rgba) = RgbaImage::from_raw(w, h, img) {
            //                 // println!("got image {}x{} w len {}", w, h, len);
            //                 gui.borrow_mut().draw_image(
            //                     &rgba,
            //                     match x {
            //                         Some(o) => numm(o),
            //                         _ => (false, 0.),
            //                     },
            //                     match y {
            //                         Some(o) => numm(o),
            //                         _ => (false, 0.),
            //                     },
            //                 );
            //             }
            //         }
            //     }
            // }

            Ok(())
        },
        "Draw image on the gui at position"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "simg",
        move |_, (name, im): (String, AnyUserData)| {
            // println!("got image {}x{} w len {}", w, h, len);
            // if let Value::UserData(imm) = im {
            if let Ok(limg) = im.borrow::<LuaImg>() {
                pitcher.send((bundle_id, MainCommmand::SetImg(name, limg.image.clone())));
            };

            Ok(())
        },
        "Draw image on the gui at position"
    );

    let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "gimg",
        move |lu, name: String| {
            //Err(mlua::prelude::LuaError::external("Failed to get image"))
            //Err(mlua::prelude::LuaError::external("Core did not respond"))
            let (tx, rx) = std::sync::mpsc::sync_channel::<(u32, u32, RgbaImage)>(0);
            let limg = match pitcher.send((bundle_id, MainCommmand::GetImg(name, tx))) {
                        Ok(o) => match rx.recv() {
                    Ok((w, h, im)) => {
                        let lua_img =
                            LuaImg::new(bundle_id, im, w, h, gui.borrow().letters.clone());
                        // table.set("w", d.0)?;
                        // table.set("h", d.1)?;
                        // table.set("data", d.2)?;
                        lua_img
                            }
                    _ => LuaImg::empty(),
                        },
                _ => LuaImg::empty(),
            };
            Ok(limg)
        },
        "Get image buffer userdata for editing"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "model",
        move |_, (name, t): (String, Table)| {
            let v = t.get::<_, Vec<[f32; 3]>>("v")?;
            if v.len() > 0 {
                let i = match t.get::<_, Vec<u32>>("i") {
                    Ok(o) => o,
                    _ => vec![],
                };
                let u = match t.get::<_, Vec<[f32; 2]>>("u") {
                    Ok(o) => o,
                    _ => vec![],
                };
                match t.get::<_, String>("t") {
                    Ok(texture) => {
                        pitcher.send((bundle_id, MainCommmand::Model(name, texture, v, i, u)));
                    }
                    _ => {
                        lg!("This type of model requires a texture at index \"t\" < t='name_of_image_without_extension' >");
                        return Ok(());
                    }
                };
            }
            Ok(())
        },
        "create a model <name:string, {v=[float,float,float][],i=int[],u=[float,float][]}>"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "lmodel",
        move |lu, (model, bundle): (String, Option<u8>)| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<String>>(0);
            pitcher.send((bundle_id, MainCommmand::ListModel(model, bundle, tx)));
            match rx.recv() {
                Ok(d) => Ok(d),
                _ => Ok(vec![]),
            }
        },
        "List models by search"
    );

    // let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "clr",
        move |_, _: ()| {
            // pitcher.send((bundle_id, MainCommmand::Clear()));
            gui.borrow_mut().clean();
            Ok(())
        },
        "Clear the gui"
    );

    lua!(
        "rnd",
        move |_, (): ()| { Ok(rand::random::<f32>()) },
        "Random"
    );
    lua!(
        "flr",
        move |_, f: f32| { Ok(f.floor() as u32) },
        "Floor value"
    );
    lua!(
        "ceil",
        move |_, f: f32| { Ok(f.ceil() as u32) },
        "Ceil value"
    );
    lua!("abs", move |_, f: f32| { Ok(f.abs()) }, "Absolute value");
    lua!("cos", move |_, f: f32| { Ok(f.cos()) }, "Cosine value");
    lua!("sin", move |_, f: f32| { Ok(f.sin()) }, "Sine value");
    lua!(
        "sqrt",
        move |_, f: f32| { Ok(f.sqrt()) },
        "Squareroot value"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "subload",
        move |_, str: String| {
            pitcher.send((bundle_id, MainCommmand::Subload(str, false)));
            Ok(())
        },
        "load a sub bundle"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "overload",
        move |_, str: String| {
            pitcher.send((bundle_id, MainCommmand::Subload(str, true)));
            Ok(())
        },
        "load an overlaying bundle"
    );

    lua!(
        "send",
        move |_, (x, y, z): (f32, f32, f32)| {
            #[cfg(feature = "online_capable")]
            {
                match &netout {
                    Some(nout) => {
                        // crate::lg!("send from com {},{}", x, y);
                        match nout.send(vec![x, y, z]) {
                            Ok(d) => {}
                            Err(e) => {
                                // println!("damn we got {}", e);
                            }
                        }
                    }
                    _ => {
                        // crate::lg!("ain't got no online");
                    }
                }
            }
            Ok(())
        },
        "Send UDP"
    );
    lua!(
        "recv",
        move |_, _: ()| {
            #[cfg(feature = "online_capable")]
            {
                match &netin {
                    Some(nin) => {
                        match nin.try_recv() {
                            Ok(r) => {
                                return Ok(r);
                                // crate::lg!("udp {:?}", r);
                            }
                            _ => {}
                        }
                    }
                    _ => {
                        // crate::lg!("ain't got no online");
                    }
                }
            }
            Ok(vec![0., 0., 0.])
        },
        "Recieve UDP"
    );

    lua!(
        "_self_destruct",
        |_, (): ()| {
            Ok(())
            //
        },
        "I guess blow up the lua core?"
    );

    // REMEMBER this always has to be at the end
    let command_map_clone = command_map.clone();
    lua!(
        "help",
        move |lu, (): ()| {
            if let Ok(t) = lu.create_table() {
                t.set("help", "list all lua commands. In fact, the command used by this program to list this very command")?;
                for (k, v) in command_map_clone.iter() {
                    t.set(k.to_string(), v.to_string())?;
                }
                Ok(t)
            } else {
                Err(mlua::Error::RuntimeError("no table".to_string()))
            }
        },
        "List all commands"
    );

    Ok(())
}

/** Error dumping helper */
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

/** core game reset, drop all resources including lua */
pub fn hard_reset(core: &mut Core) {
    core.bundle_manager.hard_reset();

    core.tex_manager.reset();
    core.model_manager.reset();
    core.gui.clean();

    core.world.destroy_it_all();
    core.global.clean();

    // TODO why doe sent reset panic?
    core.ent_manager.reset();
}

/** purge resources related to a specific bundle by id, returns true if the bundle existed */
pub fn soft_reset(core: &mut Core, bundle_id: u8) -> bool {
    let (exists, children) = core.bundle_manager.soft_reset(bundle_id);
    if exists {
        core.tex_manager.remove_bundle_content(bundle_id);
        core.tex_manager.rebuild_atlas(&mut core.world);
        // core.tex_manager.reset();
        core.model_manager.reset();
        core.gui.clean();
        core.world.destroy(bundle_id);
        core.global.clean();
        core.ent_manager.reset_by_bundle(bundle_id);
        children.iter().for_each(|c| {
            soft_reset(core, *c);
        });
        true
    } else {
        false
    }
}

pub fn load_from_string(core: &mut Core, sub_command: Option<String>) {
    load(core, sub_command, None, None, None);
}

/** Load an empty game state or bundle for issuing commands */
pub fn load_empty(core: &mut Core) {
    let bundle = core.bundle_manager.make_bundle(None, None);
    let resources = core.gui.make_morsel();
    let world_sender = core.world.make(bundle.id, core.pitcher.clone());

    bundle.lua.start(
        bundle.id,
        resources,
        world_sender,
        core.pitcher.clone(),
        core.singer.clone(),
        false,
    );
    let default = "function main() end function loop() end";
    bundle.lua.async_load(&default.to_string());
}

/**
 * Load a game from a zip file, directory, or included bytes
 * @param core
 * @param [game_path]: optional, path to either a directory of game files or a single game file
 * @param payload: included bytes, only used as part of build process
 * @param [bundle_in]: optional, if based on an existing bundle, reuse it's resources and game_path. Will ignore the game_path param
 * @param [bundle_relations]: optional, if it's attached to another bundle, either as a a sub or overlay
 */
pub fn load(
    core: &mut Core,
    game_path_in: Option<String>,
    payload: Option<Vec<u8>>,
    bundle_in: Option<u8>,
    bundle_relations: Option<(u8, bool)>,
) {
    let (game_path, bundle) = match bundle_in {
        Some(b) => {
            let bun = core.bundle_manager.bundles.get_mut(&b).unwrap();
            (bun.directory.clone(), bun)
        }
        None => (
            game_path_in.clone(),
            core.bundle_manager
                .make_bundle(game_path_in, bundle_relations),
        ),
    };
    let bundle_id = bundle.id;
    let resources = core.gui.make_morsel();
    let world_sender = core.world.make(bundle.id, core.pitcher.clone());
    bundle.lua.start(
        bundle_id,
        resources,
        world_sender,
        core.pitcher.clone(),
        core.singer.clone(),
        false,
    );
    let debug = core.global.debug;

    // TODO ensure this is reset before load
    // core.tex_manager.reset();

    // if we get a path and it's a file, it needs to be unpacked, if it's a custom directoty we walk it, otherwise walk the local directory
    match game_path {
        Some(s) => match payload {
            Some(p) => {
                crate::asset::unpack(
                    &mut core.tex_manager,
                    &mut core.model_manager,
                    &mut core.world,
                    bundle_id,
                    &bundle.lua,
                    &core.device,
                    &s,
                    p,
                    debug,
                );
                // println!("unpacked");
            }
            None => {
                let mut path = crate::asset::determine_path(Some(s.clone()));
                bundle.directory = Some(s.clone());
                if path.is_dir() {
                    crate::asset::walk_files(
                        &mut core.tex_manager,
                        &mut core.model_manager,
                        &mut core.world,
                        bundle_id,
                        Some(&core.device),
                        &bundle.lua,
                        path,
                        debug,
                    );
                } else {
                    match path.file_name() {
                        Some(file_name) => {
                            let new_path = format!("{}.game.png", file_name.to_str().unwrap_or(""));
                            // println!("it is {}", new_path);
                            drop(file_name);
                            path.set_file_name(new_path);
                            if path.is_file() {
                                let buff = crate::zip_pal::get_file_buffer_from_path(path);

                                // Some(&core.device),

                                crate::asset::unpack(
                                    &mut core.tex_manager,
                                    &mut core.model_manager,
                                    &mut core.world,
                                    bundle_id,
                                    &bundle.lua,
                                    &core.device,
                                    &s,
                                    buff,
                                    debug,
                                );
                            } else {
                                err(format!("{:?} ({}) is not a file or directory (1)", path, s));
                            }
                        }
                        None => {
                            err(format!("{} is not a file or directory (2)", s));
                        }
                    };
                }
            }
        },
        None => {
            let path = crate::asset::determine_path(None);
            crate::asset::walk_files(
                &mut core.tex_manager,
                &mut core.model_manager,
                &mut core.world,
                bundle_id,
                Some(&core.device),
                &bundle.lua,
                path,
                debug,
            );
        }
    };

    core.tex_manager
        .refinalize(&core.queue, &core.master_texture);
    // DEV  TODO
    // for e in &mut entity_manager.entities {
    //     e.hot_reload();
    // }
    let dir = match &bundle.directory {
        Some(s) => s.clone(),
        None => "_".to_string(),
    };
    log("=================================================".to_string());
    log(format!("loaded into game {}", dir));
    log("-------------------------------------------------".to_string());
    drop(bundle);
    core.update();
    core.bundle_manager.call_main(bundle_id);
}

/** reset and load previously loaded game, OR reload the binary binded game if compiled with it*/
pub fn reload(core: &mut Core, bundle_id: u8) {
    if soft_reset(core, bundle_id) {
        println!("reload from current bundle");
        load(core, None, None, Some(bundle_id), None);
    } else {
        #[cfg(feature = "include_auto")]
        {
            log("auto loading included bytes".to_string());
            let payload = include_bytes!("../auto.game.png").to_vec();
            println!("auto load bin from reload command");
            load(
                core,
                Some("INCLUDE_AUTO".to_string()),
                Some(payload),
                None,
                None,
            );
        }
        #[cfg(not(feature = "include_auto"))]
        {
            println!("reload into empty bundle");
            load(core, None, None, None, None);
        }
    }
}
// static KEYS: [String; 256] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "a", "b",
// "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u",
// "v", "w", "x", "y", "z", "escape", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9",
// "f10", "f11", "f12", "f13","f14","f15", "snap","snapshot","dele"];
fn key_match(key: String) -> usize {
    // VirtualKeyCode::from_str(&key).unwrap() as usize
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
        "escape" => 36,
        "f1" => 37,
        "f2" => 38,
        "f3" => 39,
        "f4" => 40,
        "f5" => 41,
        "f6" => 42,
        "f7" => 43,
        "f8" => 44,
        "f9" => 45,
        "f10" => 46,
        "f11" => 47,
        "f12" => 48,
        "f13" => 49,
        "f14" => 50,
        "f15" => 51,
        "f16" => 52,
        "f17" => 53,
        "f18" => 54,
        "f19" => 55,
        "f20" => 56,
        "f21" => 57,
        "f22" => 58,
        "f23" => 59,
        "f24" => 60,
        "snapshot" => 61,
        "del" => 66,
        "end" => 67,
        "pagedown" => 68,
        "pageup" => 69,
        "left" => 70,
        "up" => 71,
        "right" => 72,
        "down" => 73,
        "back" => 74,

        "return" => 75,
        // "space" => {
        //     println!("space");
        //     return 66;
        // }
        "space" => 76,
        "'" => 100,
        "apps" => 101,
        "*" => 102,
        "@" => 103,
        "ax" => 104,
        "\\" => 105,
        "calculator" => 106,
        "capital" => 107,
        ":" => 108,
        "," => 109,
        "convert" => 110,
        "=" => 111,
        "`" => 112,
        "kana" => 113,
        "kanji" => 114,
        "lalt" => 115,
        "lbracket" => 116,
        "lctrl" => 117,
        "lshift" => 118,
        "lwin" => 119,
        "mail" => 120,
        "mediaselect" => 121,
        "mediastop" => 122,
        "-" => 123,
        "mute" => 124,
        "mycomputer" => 125,
        "navigateforward" => 126,
        "navigateback" => 127,
        "nexttrack" => 128,
        "noconvert" => 129,
        "oem102" => 130,
        "." => 131,
        "playpause" => 132,
        "+" => 133,
        "power" => 134,
        "prevtrack" => 135,
        "ralt" => 136,
        "rbracket" => 137,
        "rctrl" => 138,
        "rshift" => 139,
        "rwin" => 140,
        ";" => 141,
        "/" => 142,
        "sleep" => 143,
        "stop" => 144,
        "sysrq" => 145,
        "tab" => 146,
        "_" => 147,
        "unlabeled" => 148,
        "volumedown" => 149,
        "volumeup" => 150,
        "wake" => 151,
        "webback" => 152,
        "webfavorites" => 153,
        "webforward" => 154,
        "webhome" => 155,
        "webrefresh" => 156,
        "websearch" => 157,
        "webstop" => 158,
        "yen" => 159,
        "copy" => 160,
        "paste" => 161,
        "cut" => 162,

        // "space" => VirtualKeyCode::Space,
        // "lctrl" => VirtualKeyCode::LControl,
        // "rctrl" => VirtualKeyCode::RControl,
        _ => 255,
    }
}
pub fn key_unmatch(u: usize) -> Option<char> {
    match u {
        0 => Some('1'),
        1 => Some('2'),
        2 => Some('3'),
        3 => Some('4'),
        4 => Some('5'),
        5 => Some('6'),
        6 => Some('7'),
        7 => Some('8'),
        8 => Some('9'),
        9 => Some('0'),
        10 => Some('a'),
        11 => Some('b'),
        12 => Some('c'),
        13 => Some('d'),
        14 => Some('e'),
        15 => Some('f'),
        16 => Some('g'),
        17 => Some('h'),
        18 => Some('i'),
        19 => Some('j'),
        20 => Some('k'),
        21 => Some('l'),
        22 => Some('m'),
        23 => Some('n'),
        24 => Some('o'),
        25 => Some('p'),
        26 => Some('q'),
        27 => Some('r'),
        28 => Some('s'),
        29 => Some('t'),
        30 => Some('u'),
        31 => Some('v'),
        32 => Some('w'),
        33 => Some('x'),
        34 => Some('y'),
        35 => Some('z'),
        55 => Some('e'),
        76 => Some(' '),
        100 => Some('\''),
        102 => Some('*'),
        103 => Some('@'),
        // 104=>Some("ax"),
        105 => Some('\\'),
        108 => Some(':'),
        109 => Some(','),
        111 => Some('='),
        112 => Some('`'),
        116 => Some('['),
        123 => Some('-'),
        131 => Some('.'),
        133 => Some('+'),
        137 => Some(']'),
        141 => Some(';'),
        142 => Some('/'),
        146 => Some('\t'),
        147 => Some('_'),
        _ => None,
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

/** A tuple indicating if it's to be treated as an integer (true,val), or as a float percent (false,val)*/
pub type NumCouple = (bool, f32);
pub enum MainCommmand {
    Sky(),
    Gui(),
    Fill(glam::Vec4),
    Line(NumCouple, NumCouple, NumCouple, NumCouple),
    Square(NumCouple, NumCouple, NumCouple, NumCouple),
    // Text(String, NumCouple, NumCouple),
    DrawImg(String, NumCouple, NumCouple),
    GetImg(String, SyncSender<(u32, u32, RgbaImage)>),
    SetImg(String, RgbaImage),
    Pixel(u32, u32, glam::Vec4),
    CamPos(glam::Vec3),
    CamRot(glam::Vec2),
    Clear(),
    Make(Vec<String>, SyncSender<u8>),
    Anim(String, Vec<String>, u32),
    Spawn(Arc<std::sync::Mutex<LuaEnt>>),
    Group(u64, u64, SyncSender<bool>),
    Kill(u64),
    Model(String, String, Vec<[f32; 3]>, Vec<u32>, Vec<[f32; 2]>),
    ListModel(String, Option<u8>, SyncSender<Vec<String>>),
    Globals(Vec<(String, ValueMap)>),
    AsyncError(String),
    AsyncGui(image::RgbaImage, bool),
    Reload(),
    BundleDropped(BundleResources),
    Subload(String, bool),
    WorldSync(Vec<Chunk>, bool),
    Null(),
    Stats(),
    //for testing
    Meta(usize),
}

fn decode_hex(s: &str) -> Result<Vec<u8>, core::num::ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}

fn half_decode_hex(s: &str) -> Result<Vec<u8>, core::num::ParseIntError> {
    (0..s.len())
        .map(|i| u8::from_str_radix(&s[i..i + 1], 16))
        .collect()
}

/** converts or value into a tuple indicating if it's to be treated as an integer (true,val), or as a float percent (false,val) */
fn numm(x: mlua::Value) -> NumCouple {
    match x {
        mlua::Value::Integer(i) => (true, i as f32),
        mlua::Value::Number(f) => (false, f as f32),
        _ => (false, 0.),
    }
}
fn get_color(x: mlua::Value, y: Option<f32>, z: Option<f32>, w: Option<f32>) -> Vec4 {
    match x {
        mlua::Value::String(s) => match s.to_str() {
            Ok(s2) => {
                let s = if s2.starts_with("#") {
                    &s2[1..s2.len()]
                } else {
                    s2
                };

                let halfed = s2.len() < 4;
                let res = if halfed {
                    half_decode_hex(s)
                } else {
                    decode_hex(s)
                };

                match res {
                    Ok(b) => {
                        if b.len() > 2 {
                            let f = b
                                .iter()
                                .map(|u| (*u as f32) / if halfed { 15. } else { 255. })
                                .collect::<Vec<f32>>();
                            vec4(f[0], f[1], f[2], if b.len() > 3 { f[3] } else { 1. })
                        } else {
                            vec4(0., 0., 0., 0.)
                        }
                    }
                    _ => vec4(0., 0., 0., 0.),
                }
            }
            _ => vec4(0., 0., 0., 0.),
        },
        mlua::Value::Integer(i) => vec4(
            i as f32,
            y.unwrap_or_else(|| 0.),
            z.unwrap_or_else(|| 0.),
            w.unwrap_or_else(|| 1.),
        ),
        mlua::Value::Number(f) => vec4(
            f as f32,
            y.unwrap_or_else(|| 0.),
            z.unwrap_or_else(|| 0.),
            w.unwrap_or_else(|| 1.),
        ),
        _ => vec4(1., 1., 1., 1.),
    }
}

fn table_hasher(table: mlua::Table) -> Vec<(String, ValueMap)> {
    let mut data = vec![];
    for it in table.pairs::<String, Value>() {
        if let Ok((key, val)) = it {
            println!("pair {}", key);
            let mapped = match val {
                Value::String(s) => {
                    // println!("string {}", s);
                    match s.to_str() {
                        Ok(s) => ValueMap::String(s.to_string()),
                        _ => ValueMap::Null(),
                    }
                }
                Value::Integer(i) => ValueMap::Integer(i as i32),
                Value::Number(n) => ValueMap::Float(n as f32),
                Value::Boolean(b) => ValueMap::Bool(b),
                Value::Table(t) => {
                    ValueMap::Array(
                        t.sequence_values()
                            .filter_map(|v| match v {
                                Ok(v) => match v {
                                    Value::String(s) => match s.to_str() {
                                        Ok(s) => Some(ValueMap::String(s.to_string())),
                                        _ => None,
                                    },
                                    Value::Integer(i) => Some(ValueMap::Integer(i as i32)),
                                    Value::Number(n) => Some(ValueMap::Float(n as f32)),
                                    Value::Boolean(b) => Some(ValueMap::Bool(b)),
                                    _ => None,
                                },
                                _ => None,
                            })
                            .collect::<Vec<ValueMap>>(),
                    )
                    // ValueMap::Table(table_hasher(&t, recursion_check + 1))
                }
                _ => ValueMap::Null(),
            };
            data.push((key, mapped));
        }
    }
    data
}

// #[macro_export]
// macro_rules! lg{
//     ($($arg:tt)*) => {{
//            {
//             let st=format!($($arg)*);
//             println!("{}",st);
//             crate::log::log(st);
//            }
//        }
//    }
// }

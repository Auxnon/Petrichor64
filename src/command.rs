#[cfg(feature = "headed")]
use crate::root::Core;
#[cfg(not(feature = "headed"))]
use crate::root_headless::Core;
#[cfg(feature = "audio")]
use crate::sound::{Instrument, Note, SoundCommand};
use crate::{
    bundle::BundleResources,
    gui::GuiMorsel,
    log::LogType,
    lua_define::{LuaResponse, MainPacket, SoundSender},
    lua_ent::LuaEnt,
    lua_img::{dehex, LuaImg},
    model::TextureStyle,
    online::Online,
    pad::Pad,
    tile::Chunk,
    types::ValueMap,
    world::{TileCommand, TileResponse, World},
};

use image::RgbaImage;
use itertools::Itertools;
use mlua::{
    AnyUserData, Error, Lua, Table,
    Value::{self},
    Variadic,
};
use parking_lot::Mutex;

use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::HashMap,
    rc::Rc,
    sync::{
        mpsc::{Sender, SyncSender},
        Arc,
    },
};

static com_list: [&str; 16] = [
    "new - creates a new game directory",
    "load - loads an app file",
    "pack - packs a directory into an app file",
    "unpack - unpacks an app file into a zip archive",
    "ls - show directory contents, relative to home",
    "show - open current app folder or file",
    "exit - exit to console",
    "reload - reloads game, or press Super + R",
    "atlas - dump texture atlas to png",
    "dev - toggle dev mode this session",
    "clear - clear the console",
    "bg - set console background color",
    "find - loaded assets search",
    "stats - show stats",
    "help - ;)",
    "test",
];

/** Private commands not reachable by lua code, but also works without lua being loaded */
pub fn init_con_sys(core: &mut Core, s: &str) -> bool {
    let bundle_id = core.bundle_manager.console_bundle_target;
    let main_bundle = core.bundle_manager.get_main_bundle();
    if s.len() <= 0 {
        return false;
    }
    let segments = s.trim().split(" ").collect::<Vec<&str>>();

    match segments[0] {
        "die" => {
            // this chunk could probably be passed directly to lua core but being it's significance it felt important to pass into our pre-system check for commands
            core.loggy.log(
                LogType::Config,
                &format!("killing lua instance {}", bundle_id),
            );
            main_bundle.lua.die();
        }
        "bundles" => {
            core.loggy
                .log(LogType::Config, &core.bundle_manager.list_bundles());
        }
        "pack" => {
            // new: path? name?
            // name, path, cartridge pic
            let (regular, comHash) = if segments.len() > 1 {
                getComHash(segments[1..].to_vec(), ["o", "n", "i", "c"].to_vec())
            } else {
                (vec![], HashMap::new())
            };

            let currentGameDir = main_bundle.directory.clone();
            crate::asset::pack(
                #[cfg(feature = "headed")]
                &mut core.tex_manager,
                &mut core.model_manager,
                &mut core.world,
                bundle_id,
                &main_bundle.lua,
                comHash,
                regular,
                currentGameDir,
                // &if segments.len() > 1 {
                //     format!("{}.game.png", segments[1])
                // } else {
                //     "game.png".to_string()
                // },
                // if segments.len() > 2 {
                //     Some(segments[2].to_string())
                // } else {
                //     None
                // },
                // if segments.len() > 3 {
                //     Some(segments[3].to_string())
                // } else {
                //     None
                // },
                &mut core.loggy,
                core.global.debug,
            );
        }
        "superpack" => {
            core.loggy.log(
                LogType::Config,
                &crate::asset::super_pack(&if segments.len() > 1 {
                    segments[1]
                } else {
                    "game"
                }),
            );
        }
        "unpack" => {
            if segments.len() > 1 {
                let name = segments[1];
                crate::zip_pal::unpack_and_save(
                    crate::zip_pal::get_file_buffer(&format!("./{}.game.zip", name)),
                    &format!("{}.zip", name),
                    &mut core.loggy,
                );
            } else {
                core.loggy
                    .log(LogType::ConfigError, "unpack <file without .game.png>");
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
        "exit" => {
            hard_reset(core);
            load_empty(core);
        }
        "bartholomew" => {
            hard_reset(core);
            load(
                core,
                Some("b".to_string()),
                Some(crate::asset::get_b()),
                None,
                None,
            );
        }
        "reset" => hard_reset(core),
        "reload" => reload(core, bundle_id),
        #[cfg(feature = "headed")]
        "atlas" => {
            core.tex_manager.save_atlas(&mut core.loggy);
        }
        "dev" => {
            core.global.debug = !core.global.debug;
            core.loggy
                .log(LogType::Config, &format!("debug is {}", core.global.debug));
        }
        "ls" => {
            let s = if segments.len() > 1 {
                Some(segments[1].to_string().clone())
            } else {
                None
            };
            let path = crate::asset::determine_path(s);

            match path.read_dir() {
                Ok(read) => {
                    for r in read {
                        if let Ok(e) = r {
                            if let Some(p) = e.path().file_stem() {
                                core.loggy.log(LogType::Config, &format!("{:?}", p));
                            }
                        }
                    }
                }
                Err(er) => {
                    core.loggy
                        .log(LogType::ConfigError, &format!("read error: {}", er));
                }
            }
        }
        "ugh" => {
            core.loggy.log(LogType::Sys, "heh, ya");
        }
        "clear" => core.loggy.clear(),
        "cls" => core.loggy.clear(),
        "test" => {
            core.loggy.log(LogType::Config, "that test worked, yipee");
        }
        "new" => {
            if segments.len() > 1 {
                let name = segments[1].to_string();

                let tout = main_bundle.lua.func("help(true)");
                if let LuaResponse::TableOfTuple(t) = tout {
                    for (k, (a, b)) in t.iter() {
                        println!("### {}::{}::{}", k, a, b);
                    }
                    // let mut com = vec![];
                    // let mut cur_com = "";
                    // let mut cur_desc = "";
                    // let mut alt = false;
                    // for (k, c) in t.iter() {
                    //     if !alt {
                    //         cur_com = k;
                    //         cur_desc = c;
                    //         alt = true;
                    //     } else {
                    //         com.push((cur_com.to_string(), (cur_desc.to_string(), c.to_owned())));
                    //         alt = false;
                    //     }
                    // }
                    crate::asset::make_directory(&name, &t, &mut core.loggy);
                    core.loggy
                        .log(LogType::Config, &format!("created directory {}", name));
                    hard_reset(core);
                    load(core, Some(name), None, None, None);
                } else {
                    core.loggy.log(
                        LogType::ConfigError,
                        "Problem making directory ( bad table)",
                    );
                }
            } else {
                core.loggy.log(LogType::Config, "new <name>");
            }
        }
        "show" => {
            if segments.len() > 1 {
                let name = segments[1];
                crate::asset::open_dir(name);
            } else {
                match main_bundle.directory {
                    Some(ref d) => {
                        crate::asset::open_dir(d);
                    }
                    None => {
                        crate::asset::open_dir("./");
                    }
                }
                // core.loggy.log(LogType::Config, "show <name>");
            }
        }
        "config" => {
            if segments.len() > 1 {
                let name = segments[1];
                if name.eq_ignore_ascii_case("show") {
                    crate::asset::show_config();
                }
            } else {
                if !crate::asset::check_config() {
                    crate::asset::show_config();
                } else {
                    core.global.is_state_changed = true;
                    core.global
                        .state_changes
                        .push(crate::global::StateChange::Config);
                }
            }
            // crate::asset::make_config();
        }
        "bg" => {
            if segments.len() > 1 {
                let v = dehex(segments[1]);
                #[cfg(feature = "headed")]
                core.gui.set_console_background_color(
                    (v.x * 255.) as u8,
                    (v.y * 255.) as u8,
                    (v.z * 255.) as u8,
                    (v.w * 255.) as u8,
                );
            } else {
                core.loggy.log(LogType::Config, "bg <color in hex>");
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
                            core.loggy
                                .log(LogType::Model, &format!("models -> {}", v.join(",")));
                        } else {
                            core.loggy.log(LogType::Model, "no models");
                        }
                    }
                    _ => {
                        core.loggy.log(LogType::ModelError, "???");
                    }
                }
            } else {
                core.loggy
                    .log(LogType::ModelError, "find <model | ???> <search-query>");
            }
        }
        "parse" => {
            if segments.len() > 1 {
                let s = segments[1].to_string();
                crate::parse::test(&s)
            }
        }
        "stats" => core.world.stats(),
        "help" => {
            for c in com_list {
                core.loggy.log(LogType::Config, c);
            }
        }
        &_ => return false,
    }
    true
}

pub fn init_lua_sys(
    lua_ctx: &Lua,
    lua_globals: &Table,
    bundle_id: u8,
    main_pitcher: Sender<MainPacket>,
    world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
    gui_in: Rc<RefCell<GuiMorsel>>,
    main_rast: Rc<RefCell<LuaImg>>,
    sky_rast: Rc<RefCell<LuaImg>>,
    #[cfg(feature = "headed")] singer: SoundSender,
    keys: Rc<RefCell<[bool; 256]>>,
    diff_keys: Rc<RefCell<[bool; 256]>>,
    mice: Rc<RefCell<[f32; 13]>>,
    gamepad: Rc<RefCell<Pad>>,
    ent_counter: Rc<Mutex<u64>>,
    loggy: Sender<(LogType, String)>,
) -> Result<(), Error>
// where N: 
// #[cfg(feature = "online_capable")]
// Option<(Sender<MovePacket>, Receiver<MovePacket>)> 
// #[cfg(not(feature = "online_capable"))]
// Option<bool>
{
    println!("init lua sys");

    let default_func = lua_ctx
        .create_function(|_, _: f32| Ok("placeholder func uwu"))
        .unwrap();
    res(
        "_default_func",
        lua_globals.set("_default_func", default_func),
        &loggy,
    );

    let mut command_map: Vec<(String, (String, String))> = vec![];

    lua_globals.set("pi", std::f64::consts::PI);
    lua_globals.set("tau", std::f64::consts::PI * 2.0);
    lua_globals.set("gui", main_rast);
    lua_globals.set("sky", sky_rast);

    // lua_ctx.set_warning_function(|a, b, f| {
    //     log(format!("hi {:?}", b));
    //     Ok(())
    // });

    #[macro_export]
    macro_rules! lua {
        ($name:expr,$closure:expr,$desc:expr,$exam:expr) => {
            #[cfg(debug_assertions)]
            {
                let _f = include_bytes!(concat!("../guide/", $name, ".md"));
                // read to string
                // let st = std::str::from_utf8(f).unwrap();
                // let ar = st.split("```").collect::<Vec<&str>>();
                // if ar.len() > 2 {
                //     let s = ar[1].to_string();
                //     let exam = s.trim();
                //     assert_eq!(exam, $exam);
                // }
            }
            command_map.push(($name.to_string(), ($desc.to_string(), $exam.to_string())));
            res(
                $name,
                lua_globals.set($name, lua_ctx.create_function($closure).unwrap()),
                &loggy,
            );
        };
    }

    let aux_loggy = loggy.clone();
    lua!(
        "cout",
        move |_, args: Variadic<String>| {
            aux_loggy.send((LogType::Lua, args.iter().join(", ")));
            Ok(())
        },
        "Prints string to console",
        "
---@param message string
function cout(message) end"
    );

    let sender = world_sender.clone();
    lua!(
        "tile",
        move |_, (t, x, y, z, r): (Value, i32, i32, i32, Option<u8>)| {
            let tile = match t {
                Value::String(s) => s.to_str().unwrap_or("").to_string(),
                _ => "".to_string(),
            };
            let ro = match r {
                Some(i) => i,
                None => 0,
            };

            World::set_tile(&sender, tile, x, y, z, ro);
            Ok(1)
        },
        "Set a tile within 3d space. Nil asset deletes.",
        "
---@param asset string
---@param x integer
---@param y integer
---@param z integer
---@param rot integer?
function tile(asset, x, y, z, rot) end"
    );

    let sender = world_sender.clone();
    lua!(
        "dtile",
        move |_, (x, y, z): (Option<i32>, Option<i32>, Option<i32>)| {
            match (x, y, z) {
                (xx,yy,zz) => World::drop_chunk(&sender, xx.unwrap_or(0), yy.unwrap_or(0), zz.unwrap_or(0)),
                (None,None,None) => World::clear_tiles(&sender),
            }
            Ok(1)
        },
        "Crude deletion of a 16x16x16 chunk. Extremely efficient for large area tile changes. Not including arguments delete all tiles.",
        "
---@param x? integer
---@param y? integer
---@param z? integer
function dtile( x, y, z) end"
    );

    // MARK
    // BLUE TODO this function is expensive? if called twice in one cycle it ruins key press checks??
    let sender = world_sender.clone();
    lua!(
        "istile",
        move |_, (x, y, z): (i32, i32, i32)| { Ok(World::is_tile(&sender, x, y, z)) },
        "Check if a tile is present at a given location",
        "
---@param x integer 
---@param y integer
---@param z integer
---@return boolean
function istile(x, y, z) end"
    );

    let sender = world_sender.clone();
    lua!(
        "gtile",
        move |_, (x, y, z): (i32, i32, i32)| {
            let t = World::get_tile(&sender, x, y, z);
            Ok(match t {
                Some(s) => s.0,
                None => "".to_string(),
            })
        },
        "Return the asset name of the tile at a given location",
        "
---@param x integer 
---@param y integer
---@param z integer
---@return string
function gtile(x, y, z) end"
    );
    let sender = world_sender.clone();
    lua!(
        "ftile",
        move |l, (t, x, y, z, dx, dy, dz): (String, i32, i32, i32, i32, i32, i32)| {
            let tt = if t.len() == 0 { None } else { Some(t) };
            match World::first_tile(&sender, tt, x, y, z, dx, dy, dz, 100) {
                Some(v) => l.create_table_from(vec![(0, v[0]), (1, v[1]), (2, v[2])].into_iter()),
                None => {
                    let f: Vec<(usize, i32)> = vec![];
                    l.create_table_from(f.into_iter())
                }
            }
        },
        "Find first occurence of a tile in a given direction",
        "
---@param x integer 
---@param y integer
---@param z integer
---@return string
function gtile(x, y, z) end"
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
        "Set an animation by passing in series of textures",
        "   
---@param name string
---@param items string[]
---@param speed number?
function anim(name, items, speed) end"
    );

    let dkeys = diff_keys.clone();
    lua!(
        "key",
        move |_, (key, volatile): (String, Option<bool>)| {
            match volatile {
                Some(true) => Ok(dkeys.borrow()[key_match(key)]),
                _ => Ok(keys.borrow()[key_match(key)]),
            }
        },
        "Check if key is held down",
        "   
---@param key string
---@param volatile boolean? only true on first frame of key press
---@return boolean
function key(key, volatile) end"
    );

    lua!(
        "cin",
        move |_, _: ()| {
            let h: String = diff_keys
                .borrow()
                .iter()
                .enumerate()
                .filter_map(|(i, k)| if *k { key_unmatch(i) } else { None })
                .collect();

            Ok(h)
        },
        "Get a string of all keys pressed",
        "   
---@return string
function cin() end"
    );

    lua!(
        "mus",
        move |lu, (): ()| {
            let t = lu.create_table()?;
            let m = mice.borrow();
            t.set("x", m[0])?;
            t.set("y", m[1])?;
            t.set("dx", m[2])?;
            t.set("dy", m[3])?;
            t.set("px", m[4])?;
            t.set("py", m[5])?;

            // t.set("z",m[2])?;
            t.set("m1", m[6] > 0.)?;
            t.set("m2", m[7] > 0.)?;
            t.set("m3", m[8] > 0.)?;
            t.set("scroll", m[9])?;

            t.set("vx", m[10])?;
            t.set("vy", m[11])?;
            t.set("vz", m[12])?;

            Ok(t)
        },
        " Get mouse position, delta, button states, and unprojected vector",
        "
---@return mouse
function mus() end"
    );

    let gam = Rc::clone(&gamepad);
    lua!(
        "btn",
        move |_, button: String| { Ok(gam.borrow().check(button) != 0.) },
        "Check if gamepad button is held down",
        "
---@param button string
---@return boolean
function btn(button) end"
    );

    lua!(
        "abtn",
        move |_, button: String| { Ok(gamepad.borrow().check(button)) },
        "Check how much a gamepad is pressed, axis gives value between -1 and 1",
        "
---@param button string
---@return number
function abtn(button) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "make",
        move |_, (asset, x, y, z, s): (String, f64, f64, f64, Option<f64>)| {
            // let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<Arc<std::sync::Mutex<LuaEnt>>>>(0);
            let id = *ent_counter.lock();
            *ent_counter.lock() += 1;

            let ent = crate::lua_ent::LuaEnt::new(id, asset, x, y, z, s.unwrap_or(1.));
            let wrapped = Arc::new(std::sync::Mutex::new(ent));

            // match pitcher.send(MainCommmand::Spawn(asset, x, y, z, s.unwrap_or(1.), 1, tx)) {

            match pitcher.send((bundle_id, MainCommmand::Spawn(Arc::clone(&wrapped)))) {
                Ok(_) => {}
                Err(_) => return Err(make_err("Unable to create entity")),
            }

            // Ok(match rx.recv() {
            //     Ok(mut e) => e.remove(0),
            //     Err(e) => Arc::new(std::sync::Mutex::new(LuaEnt::empty())),
            // })
            Ok(wrapped)
        },
        "Spawn an entity from an asset",
        "
---@param asset string
---@param x number
---@param y number
---@param z number
---@param scale number?
---@return entity
function make(asset, x, y, z, scale) end"
    );
    let pitcher = main_pitcher.clone();
    lua!(
        "lot",
        move |_, (parent,child ): (Arc<std::sync::Mutex<LuaEnt>>,Arc<std::sync::Mutex<LuaEnt>>)| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<bool>(0);
            let parentId=parent.lock().unwrap().get_id();
            let childId=child.lock().unwrap().get_id();
            match pitcher.send((bundle_id,MainCommmand::Group(parentId,childId, tx))) {
                Ok(_) => {}
                Err(er) => {
                   return Err(make_err("Unable to group entity"));
                },
            };
            match rx.recv(){
                Ok(_) => {},
                Err(_) => {}
            };

            Ok(())
        },
        "Groups an entity onto another entity",
        "
---@param parent entity
---@param child entity
function lot(parent, child) end"
    );

    lua!(
        "kill",
        move |_, ent: Arc<std::sync::Mutex<LuaEnt>>| {
            if let Ok(mut r) = ent.lock() {
                r.kill();
            }
            Ok(())
        },
        "Removes an entity",
        "
---@param ent entity | integer
function kill(ent) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "init",
        move |_, (): ()| {
            // println!("hit reset");
            match pitcher.send((bundle_id, MainCommmand::Reload())) {
                Ok(_) => {}
                Err(er) => {}
            }
            Ok(())
        },
        "Resets lua context and reloads all assets and scripts fresh",
        "
function reload() end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "attr",
        move |_, table: Table| {
            let hash = table_hasher(table);
            pitcher.send((bundle_id, MainCommmand::Globals(hash)));
            Ok(())
        },
        "Set various app state parameters",
        "
---@param attributes Attributes
function attr(attributes) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "cam",
        move |_, table: Table| {
            let pos = match table.get("pos") {
                Ok(v) => match v {
                    Value::Table(t) => {
                        let x = t.get::<_, f32>(1).unwrap_or(0.);
                        let y = t.get::<_, f32>(2).unwrap_or(0.);
                        let z = t.get::<_, f32>(3).unwrap_or(0.);
                        Some(glam::vec3(x, y, z))
                    }
                    _ => None,
                },
                _ => None,
            };
            let rot = match table.get("rot") {
                Ok(v) => match v {
                    Value::Table(t) => {
                        let x = t.get::<_, f32>(1).unwrap_or(0.);
                        let y = t.get::<_, f32>(2).unwrap_or(0.);
                        Some(glam::vec2(x, y))
                    }
                    _ => None,
                },
                _ => None,
            };

            pitcher.send((bundle_id, MainCommmand::Cam(pos, rot)));

            Ok(())
        },
        "Set the camera position and/or rotation",
        "
---@param params cam_params
function cam(params) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "note",
        move |_, (freq, length): (f32, Option<f32>)| {
            #[cfg(feature = "audio")]
            {
                let len = match length {
                    Some(l) => l,
                    None => 1.,
                };
                sing.send(SoundCommand::PlayNote(Note::new(0, freq, len, 1.), None));
            }
            Ok(())
        },
        "Make a sound or note",
        "
---@param freq number
---@param length number?
function sound(freq, length) end"
    );
    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "song",
        move |_, (notes): (Vec<Value>)| {
            #[cfg(feature = "audio")]
            {
                let converted = notes
                    .iter()
                    .filter_map(|v| match v {
                        Value::Table(t) => {
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
                        Value::Integer(n) => Some(Note::new(0, *n as f32, 1., 1.)),
                        _ => None,
                    })
                    .collect::<Vec<Note>>();

                sing.send(SoundCommand::Chain(converted, None));
            }
            Ok(())
        },
        "Make a song",
        "   
---@param notes number[][] | number[] nested array first is frequency, second is length
function song(notes) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "mute",
        move |_, (channel): (Option<usize>)| {
            #[cfg(feature = "audio")]
            sing.send(SoundCommand::Stop(channel.unwrap_or((0))));

            Ok(())
        },
        "Stop sounds on channel",
        "
---@param channel number
function mute(channel) end"
    );

    lua!(
        "instr",
        move |_, (freqs, half): (Vec<f32>, Option<bool>)| {
            #[cfg(feature = "audio")]
            singer.send(SoundCommand::MakeInstrument(Instrument::new(
                0,
                freqs,
                match half {
                    Some(h) => h,
                    None => false,
                },
            )));

            Ok(())
        },
        "Make an instrument",
        "
---@param freqs number[]
---@param half boolean? subsequent freqs are half the previous  
function instr(freqs, half) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "tex",
        move |_, (name, im): (String, AnyUserData)| {
            if let Ok(limg) = im.borrow::<LuaImg>() {
                pitcher.send((bundle_id, MainCommmand::SetImg(name, limg.image.clone())));
            };

            Ok(())
        },
        "Sets image data as a texture",
        "
---@param asset string
---@param im image
function tex(asset, im) end"
    );

    let pitcher = main_pitcher.clone();
    let gui = gui_in.clone();
    lua!(
        "gimg",
        move |lu, name: String| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<(u32, u32, RgbaImage)>(0);
            let limg = match pitcher.send((bundle_id, MainCommmand::GetImg(name, tx))) {
                Ok(o) => match rx.recv() {
                    Ok((w, h, im)) => {
                        let lua_img =
                            LuaImg::new(bundle_id, im, w, h, gui.borrow().letters.clone());
                        lua_img
                    }
                    _ => LuaImg::empty(),
                },
                _ => LuaImg::empty(),
            };
            Ok(limg)
        },
        "Get image buffer userdata for editing or drawing",
        "
---@param asset string  
---@return image
function gimg(asset) end"
    );

    let gui = gui_in.clone();
    lua!(
        "nimg",
        move |_, (w, h): (u32, u32)| {
            let im = GuiMorsel::new_image(w, h);
            let lua_img = LuaImg::new(bundle_id, im, w, h, gui.borrow().letters.clone());

            Ok(lua_img)
        },
        "Create new image buffer userdata, does not set as asset",
        "
---@param w integer
---@param h integer
---@return image
function nimg(w, h) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
            "mod",
            move |_, (name, t): (String, Table)| {
                let (tx, rx) = std::sync::mpsc::sync_channel::<u8>(0);

                match t.get::<_, Vec<[f32; 3]>>("q") {
                    Ok(quads) => {

                        let (v,n, uv, i) =
                        match t.get::<_, Vec<[f32;2]>>("u") {
                            Ok(uvi)=>{
                                let inds= t.get::<_, Vec<u32>>("i").unwrap_or_else(|_|{
                                    let mut ind = Vec::new();
                                    let mut i = 0;
                                    for _ in 0..quads.len() {
                                        ind.push(i as u32);
                                        ind.push((i + 1) as u32);
                                        ind.push((i + 2) as u32);
                                        ind.push((i + 2) as u32);
                                        ind.push((i + 3) as u32);
                                        ind.push(i as u32);
                                        i += 4;
                                    }
                                    ind
                                });
                                let norms=t.get::<_, Vec<[f32;3]>>("n").unwrap_or_else(|_|{
                                    let mut norms = Vec::new();
                                    for _ in 0..quads.len() {
                                        norms.push([0., 0., 0.]);
                                        norms.push([0., 0., 0.]);
                                        norms.push([0., 0., 0.]);
                                        norms.push([0., 0., 0.]);
                                    }
                                    norms
                                });
                                (quads,norms, uvi, inds)


                            }
                            _=>{
    convert_quads(quads)
                            }
                        };
                        match t.get::<_, Vec<String>>("t") {
                            Ok(texture) => {
                                pitcher.send((
                                    bundle_id,
                                    MainCommmand::Model(
                                        name,
                                        texture,
                                        v,
                                        n,
                                        i,
                                        uv,
                                        TextureStyle::Quad,
                                        tx,
                                    ),
                                ));
                                if let Err(err) = rx.recv() {
                                    return Err(make_err(&err.to_string()));
                                }
                            }
                            _ => {
                                Err::<(), &str>("This type of model requires a texture");
                            }
                        }
                        Ok("Building model in quad mode")
                    }
                    _ => {
                        // println!("got no quads");
                        let vin = t.get::<_, Vec<[f32; 3]>>("v");
                        match vin {
                            Ok(v) => {
                                if v.len() > 2 {
                                    let i = match t.get::<_, Vec<u32>>("i") {
                                        Ok(o) => o,
                                        _ => vec![],
                                    };
                                    let u = match t.get::<_, Vec<[f32; 2]>>("u") {
                                        Ok(o) => o,
                                        _ => vec![],
                                    };
                                    let n = match t.get::<_, Vec<[f32; 3]>>("n") {
                                        Ok(o) => o,
                                        _ => vec![],
                                    };
                                    match t.get::<_, Vec<String>>("t") {
                                        Ok(texture) => {
                                            pitcher.send((
                                                bundle_id,
                                                MainCommmand::Model(
                                                    name,
                                                    texture,
                                                    v,
                                                    n,
                                                    i,
                                                    u,
                                                    TextureStyle::Tri,
                                                    tx,
                                                ),
                                            ));
                                            if let Err(err) = rx.recv() {
                                                return Err(make_err(&err.to_string()));
                                            }
                                            return Ok("Building model in vert mode")
                                        }
                                        _ => {
                                            return Err(make_err("This type of model requires a texture at index \"t\" < t='name_of_image_without_extension' >"))
                                            // return Ok(());
                                        }
                                    }
                                }
                                Err(make_err("This type of model requires a vertex list at index \"v\" < v={{0,0,0},{1,0,0},{1,1,0},{0,1,0}} >"))
                            }
                            _ => {
                                match t.get::<_, Vec<String>>("t") {
                                    Ok(texture) => {
                                        if texture.len() > 0 {
                                            let t = texture[0].clone();
                                            pitcher.send((
                                                bundle_id,
                                                MainCommmand::Make(
                                                    vec![
                                                        name,
                                                        t.clone(),
                                                        texture.get(1).unwrap_or(&t).to_string(),
                                                        texture.get(2).unwrap_or(&t).to_string(),
                                                        texture.get(3).unwrap_or(&t).to_string(),
                                                        texture.get(4).unwrap_or(&t).to_string(),
                                                        texture.get(5).unwrap_or(&t).to_string(),
                                                    ],
                                                    tx,
                                                ),
                                            ));
                                            if let Err(err) = rx.recv() {
                                                return Err(make_err(&err.to_string()));
                                            }
                                        }
                                        Ok("Building model in texture mode")
                                    }
                                    _ => {
                                        // Err::<(),&str>
                                        Err(make_err("This type of model requires a texture at index \"t\" < t='name_of_image_without_extension' >"))
                                    }
                                }
                            }
                        }
                    }
                }

            },
            "insert model data into an asset <name:string, {v=[float,float,float][],i=int[],u=[float,float][]}>",
            "
---@param asset string
---@param t model_data
---@return string stating what mode the model was built in
function mod(asset, t) end"
        );

    let pitcher = main_pitcher.clone();
    lua!(
        "gmod",
        move |_, (model, bundle): (Option<String>, Option<u8>)| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<String>>(0);
            pitcher.send((
                bundle_id,
                MainCommmand::ListModel(model.unwrap_or("".to_string()), bundle, tx),
            ));
            match rx.recv() {
                Ok(d) => Ok(d),
                _ => Ok(vec![]),
            }
        },
        "List models by search",
        "
---@param model string  
---@param bundle integer?
---@return string[]
function lmod(model, bundle) end"
    );

    //     let gui = gui_in.clone();
    //     lua!(
    //         "clr",
    //         move |_, _: ()| {
    //             gui.borrow_mut().clean();
    //             Ok(())
    //         },
    //         "Clear the draw target",
    //         "
    // function clr() end"
    //     );

    // lua!(
    //     "cam",
    //     move |_, (x, y, z, rx, ry, rz, fov, near, far): (f32, f32, f32, f32, f32, f32, f32, f32, f32)| {
    //         pitcher.send((bundle_id, MainCommmand::Camera(x, y, z, rx, ry, rz, fov, near, far)));
    //         O    k(())
    //     },
    //     "Set the camera position and rotation",
    //     ""
    // );

    // TODO modulo bias?
    // let mut rng = rand::thread_rng();
    lua!(
        "rnd",
        move |_, (a, b): (Option<f32>, Option<f32>)| {
            match a {
                Some(fa) => match b {
                    Some(fb) => Ok(rand::random::<f32>() * (fb - fa) + fa),
                    _ => Ok(rand::random::<f32>() * fa),
                },
                _ => Ok(rand::random::<f32>()),
            }
        },
        "Random float from 0-1, or provide a range",
        "
---@param a number?
---@param b number?
---@return number
function rnd(a, b) end"
    );

    lua!(
        "irnd",
        move |_, (a, b): (Option<i32>, Option<i32>)| {
            match a {
                Some(fa) => match b {
                    Some(fb) => Ok((rand::random::<f32>() * (fb - fa) as f32).floor() as i32 + fa),
                    _ => Ok((rand::random::<f32>() * fa as f32).floor() as i32),
                },
                _ => Ok(rand::random::<i32>()),
            }
        },
        "An imperfect random number generator for integers. May suffer from modulo bias, only i32",
        "
---@param a integer?
---@param b integer?
---@return integer
function irnd(a, b) end"
    );

    lua!(
        "flr",
        move |_, f: f32| { Ok(f.floor() as i32) },
        "Floor value",
        "
---@param f number
---@return integer
function flr(f) end"
    );

    lua!(
        "ceil",
        move |_, f: f32| { Ok(f.ceil() as i32) },
        "Ceil value",
        "
---@param f number
---@return integer
function ceil(f) end"
    );

    lua!(
        "abs",
        move |_, f: f32| { Ok(f.abs()) },
        "Absolute value",
        "
---@param f number
---@return number
function abs(f) end"
    );

    lua!(
        "cos",
        move |_, f: f32| { Ok(f.cos()) },
        "Cosine value",
        "
---@param f number  
---@return number
function cos(f) end"
    );
    lua!(
        "sin",
        move |_, f: f32| { Ok(f.sin()) },
        "Sine value",
        "
---@param f number
---@return number
function sin(f) end"
    );
    lua!(
        "sqrt",
        move |_, f: f32| { Ok(f.sqrt()) },
        "Squareroot value",
        "
---@param f number
---@return number
function sqrt(f) end"
    );

    lua!(
        "pow",
        move |_, (f, e): (f32, f32)| { Ok(f.powf(e)) },
        "Squareroot value",
        "
---@param f number target
---@param e number raise by
---@return number
function pow(f,e) end"
    );
    lua!(
        "log",
        move |_, f: f32| { Ok(f.log10()) },
        "Base 10 logarithm of the value",
        "
---@param f number target
---@return number
function log(f) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "sub",
        move |_, str: String| {
            pitcher.send((bundle_id, MainCommmand::Subload(str, false)));
            Ok(())
        },
        "Load a sub bundle",
        "
---@param str string
function sub(str) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "over",
        move |_, str: String| {
            pitcher.send((bundle_id, MainCommmand::Subload(str, true)));
            Ok(())
        },
        "load an overlaying bundle",
        "   
---@param str string
function over(str) end"
    );

    lua!(
        "conn",
        move |_, (addr, udp, server): (String, bool, bool)| {
            #[cfg(feature = "online_capable")]
            {
                return match _net_sender.borrow_mut().open(&addr, udp, server) {
                    Ok(c) => Ok(c),
                    Err(er) => Err(make_err(&format!("Unable to create connection {}", er))),
                };
            }
            #[cfg(not(feature = "online_capable"))]
            Ok(())
        },
        "Create a new connection to the ip, site, etc. A :port is optional",
        "
--- @param addr string
--- @return connection
function conn(addr) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "quit",
        move |_, u: Option<u8>| {
            pitcher.send((bundle_id, MainCommmand::Quit(u.unwrap_or(0))));
            Ok(())
        },
        "Hard quit or exit to console",
        "
---@param u integer? >0 soft quits
function quit(u) end"
    );

    //     lua!(
    //         "error",
    //         move |_, (): ()| { Ok(()) },
    //         "Get current errors for a bundle",
    //         "
    // ---@return string
    // function error() end"
    //     );

    // REMEMBER this always has to be at the end
    let command_map_clone = command_map.clone();
    lua!(
        "help",
        move |lu, b: bool| {
            if let Ok(t) = lu.create_table() {
                t.set("help", "list all lua commands. In fact, the command used by this program to list this very command")?;
                for (k, (desc, examp)) in command_map_clone.iter() {
                    if b {
                        t.set(k.to_string(), [desc.to_string(), examp.to_string()])?;
                    } else {
                        t.set(k.to_string(), desc.to_string())?;
                    }
                }
                Ok(t)
            } else {
                Err(mlua::Error::RuntimeError("no table".to_string()))
            }
        },
        "List all commands",
        "
---@return table
function help() end"
    );

    lua_ctx
        .load(
            "
        add=table.insert 
        del=table.remove 
        print=cout 
        fill = function(...) gui:fill(...) end
        rect = function(...) gui:rect(...) end
        rrect = function(...) gui:rrect(...) end
        line = function(...) gui:line(...) end
        text = function(...) gui:text(...) end
        img = function(...) gui:img(...) end
        clr = function(...) gui:clr(...) end
        ",
        )
        .exec()?;

    Ok(())
}

/** Error dumping helper */
fn res(target: &str, r: Result<(), Error>, loggy: &Sender<(LogType, String)>) {
    match r {
        Err(err) => {
            loggy.send((
                LogType::LuaSysError,
                format!(
                    "🔴lua::problem setting default lua function {}, {}",
                    target, err
                ),
            ));
        }
        _ => {}
    }
}

/** core game reset, drop all resources including lua */
pub fn hard_reset(core: &mut Core) {
    core.bundle_manager.hard_reset();

    #[cfg(feature = "headed")]
    core.tex_manager.reset();
    core.model_manager.reset();
    // core.gui.clean(); //TODO

    core.world.destroy_it_all();
    core.global.clean();

    // TODO why doe sent reset panic?
    core.ent_manager.reset();
}

/** purge resources related to a specific bundle by id, returns true if the bundle existed */
pub fn soft_reset(core: &mut Core, bundle_id: u8) -> bool {
    let (exists, children) = core.bundle_manager.soft_reset(bundle_id);
    if exists {
        #[cfg(feature = "headed")]
        core.tex_manager.remove_bundle_content(bundle_id);
        #[cfg(feature = "headed")]
        core.tex_manager
            .rebuild_atlas(&mut core.world, &mut core.loggy);
        core.model_manager.reset();
        // core.gui.clean(); // TODO
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
    // skip_logo: bool
    let bundle = core.bundle_manager.make_bundle(None, None);
    let resources = core.gui.make_morsel();
    let world_sender = core.world.make(bundle.id, core.pitcher.clone());
    bundle.lua_ctx_handle = Some(bundle.lua.start(
        bundle.id,
        resources,
        world_sender,
        core.pitcher.clone(),
        core.loggy.make_sender(),
        #[cfg(feature = "audio")]
        core.singer.clone(),
        core.global.debug,
        false,
    ));

    #[cfg(feature = "headed")]
    {
        let payload = crate::asset::get_logo();
        crate::asset::unpack(
            #[cfg(feature = "headed")]
            &core.gfx.device,
            #[cfg(feature = "headed")]
            &mut core.tex_manager,
            &mut core.model_manager,
            &mut core.world,
            bundle.id,
            &bundle.lua,
            "empty",
            payload,
            &mut core.loggy,
            core.global.debug,
        );
    }

    #[cfg(feature = "headed")]
    core.tex_manager
        .refinalize(&core.gfx.queue, &core.gfx.master_texture);
    // DEV  TODO

    // let default = "function main() end function loop() end";
    // bundle.lua.async_load(&default.to_string());
    bundle.call_main();
    core.global.boot_state = true;
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

    bundle.lua_ctx_handle = Some(bundle.lua.start(
        bundle_id,
        resources,
        world_sender,
        core.pitcher.clone(),
        core.loggy.make_sender(),
        #[cfg(feature = "audio")]
        core.singer.clone(),
        core.global.debug,
        false,
    ));

    let debug = core.global.debug;

    // TODO ensure this is reset before load
    // core.tex_manager.reset();

    // if we get a path and it's a file, it needs to be unpacked, if it's a custom directoty we walk it, otherwise walk the local directory
    match game_path {
        Some(s) => match payload {
            Some(p) => {
                crate::asset::unpack(
                    #[cfg(feature = "headed")]
                    &core.gfx.device,
                    #[cfg(feature = "headed")]
                    &mut core.tex_manager,
                    &mut core.model_manager,
                    &mut core.world,
                    bundle_id,
                    &bundle.lua,
                    &s,
                    p,
                    &mut core.loggy,
                    debug,
                );
                // println!("unpacked");
            }
            None => {
                let mut path = crate::asset::determine_path(Some(s.clone()));
                bundle.directory = Some(s.clone());
                if path.is_dir() {
                    crate::asset::walk_files(
                        #[cfg(feature = "headed")]
                        Some(&core.gfx.device),
                        #[cfg(feature = "headed")]
                        &mut core.tex_manager,
                        &mut core.model_manager,
                        true,
                        &mut core.world,
                        bundle_id,
                        &bundle.lua,
                        path,
                        &mut core.loggy,
                        debug,
                    );
                } else {
                    match path.file_name() {
                        Some(file_name) => {
                            let ff = file_name.to_str().unwrap_or("");
                            let new_path = if ff.contains(".") {
                                ff.to_string()
                            } else {
                                format!("{}.game.png", ff)
                            };

                            // println!("it is {}", new_path);
                            drop(file_name);
                            path.set_file_name(new_path);
                            if path.is_file() {
                                let buff = crate::zip_pal::get_file_buffer_from_path(path);

                                // Some(&core.device),

                                crate::asset::unpack(
                                    #[cfg(feature = "headed")]
                                    &core.gfx.device,
                                    #[cfg(feature = "headed")]
                                    &mut core.tex_manager,
                                    &mut core.model_manager,
                                    &mut core.world,
                                    bundle_id,
                                    &bundle.lua,
                                    &s,
                                    buff,
                                    &mut core.loggy,
                                    debug,
                                );
                            } else {
                                core.loggy.log(
                                    LogType::ConfigError,
                                    &format!("{:?} ({}) is not a file or directory (1)", path, s),
                                );
                            }
                        }
                        None => {
                            core.loggy.log(
                                LogType::ConfigError,
                                &format!("{} is not a file or directory (2)", s),
                            );
                        }
                    };
                }
            }
        },
        None => {
            let path = crate::asset::determine_path(None);
            crate::asset::walk_files(
                #[cfg(feature = "headed")]
                Some(&core.gfx.device),
                #[cfg(feature = "headed")]
                &mut core.tex_manager,
                &mut core.model_manager,
                true,
                &mut core.world,
                bundle_id,
                &bundle.lua,
                path,
                &mut core.loggy,
                debug,
            );
        }
    };
    #[cfg(feature = "headed")]
    core.tex_manager
        .refinalize(&core.gfx.queue, &core.gfx.master_texture);
    // DEV  TODO
    // for e in &mut entity_manager.entities {
    //     e.hot_reload();
    // }
    let dir = match &bundle.directory {
        Some(s) => s.clone(),
        None => "_".to_string(),
    };
    core.loggy.log(
        LogType::Config,
        "=================================================",
    );
    core.loggy
        .log(LogType::Config, &format!("loaded into game {}", dir));
    core.loggy.log(
        LogType::Config,
        "-------------------------------------------------",
    );
    // drop(bundle);
    // TODO do we need to run an update here?
    // core.update();
    core.loggy.log(LogType::Config, "calling main method");
    // core.bundle_manager.call_main(bundle_id);
    bundle.call_main();
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

/** A tuple indicating if it's to be treated as an integer (true,val), or as a float percent (false,val)*/
pub type NumCouple = (bool, f32);
pub enum MainCommmand {
    GetImg(String, SyncSender<(u32, u32, RgbaImage)>),
    SetImg(String, RgbaImage),
    Cam(Option<glam::Vec3>, Option<glam::Vec2>),
    Make(Vec<String>, SyncSender<u8>),
    Anim(String, Vec<String>, u32),
    Spawn(Arc<std::sync::Mutex<LuaEnt>>),
    Group(u64, u64, SyncSender<bool>),
    Model(
        String,
        Vec<String>,
        Vec<[f32; 3]>,
        Vec<[f32; 3]>,
        Vec<u32>,
        Vec<[f32; 2]>,
        TextureStyle,
        SyncSender<u8>,
    ),
    ListModel(String, Option<u8>, SyncSender<Vec<String>>),
    Globals(Vec<(String, ValueMap)>),
    AsyncError(String),
    LoopComplete((Option<image::RgbaImage>, Option<image::RgbaImage>)),
    Reload(),
    BundleDropped(BundleResources),
    Load(String),
    Subload(String, bool),
    WorldSync(Vec<Chunk>, bool),
    Null(),
    Stats(),
    //for testing
    Meta(crate::gui::ScreenIndex),
    Quit(u8),
}

pub fn num(x: Value) -> LuaResponse {
    match x {
        Value::Integer(i) => LuaResponse::Integer(i),
        Value::Number(f) => LuaResponse::Number(f),
        Value::String(s) => match s.to_str() {
            Ok(s) => LuaResponse::String(s.to_string()),
            _ => LuaResponse::Integer(0),
        },
        _ => LuaResponse::Integer(0),
    }
}
pub fn numop(x: Option<Value>) -> LuaResponse {
    match x {
        Some(v) => num(v),
        _ => LuaResponse::Integer(0),
    }
}
fn nummold(x: mlua::Value) -> NumCouple {
    match x {
        mlua::Value::Integer(i) => (true, i as f32),
        mlua::Value::Number(f) => (f >= 2., f as f32),
        _ => (false, 0.),
    }
}

/** converts or value into a tuple indicating if it's to be treated as an integer (true,val), or as a float percent (false,val) */
// fn numm2(x: mlua::Value) -> GuiUnit {
//     match x {
//         // mlua::Value::Integer(i) => (true, i as f32),
//         // mlua::Value::Number(f) => (f >= 2., f as f32),
//         // _ => (false, 0.),
//         Value::Integer(i) => {
//             if i < 0 {
//                 GuiUnit::ReversePixel(i.abs() as u32)
//             } else {
//                 GuiUnit::Pixel(i as u32)
//             }
//         }
//         Value::Number(f) => {
//             if f < 0. {
//                 GuiUnit::ReversePercent(f.abs() as f32)
//             } else {
//                 GuiUnit::Percent(f as f32)
//             }
//         }
//         Value::String(s) => match s.to_str() {
//             Ok(s) => {
//                 // print!()
//                 let st = s.trim();
//                 if st.starts_with("=") {
//                     st.split(['-', '+']).for_each(|p| {
//                         let seg = p.trim();
//                         // if st.ends_with("%") {
//                         //     if st.ends_with("@%") {
//                         //         let n = st[0..st.len() - 2].parse::<f32>().unwrap_or(0);
//                         //         return GuiUnit::AspectPercent(n / 100.);
//                         //     } else {
//                         //         let n = st.parse::<f32>().unwrap_or(0.);
//                         //         return GuiUnit::Percent(n / 100.);
//                         //     }
//                         // } else if st.ends_with("@") {
//                         //     let n = st.parse::<u32>().unwrap_or(0);
//                         //     return GuiUnit::Pixel(n);
//                         // }

//                         // if s.starts_with("=") {
//                         //     let n = s[1..].parse::<f32>().unwrap_or(0.);
//                         //     return GuiUnit::AspectPercent(n / 100.);
//                         // } else {
//                         //     let n = s.parse::<f32>().unwrap_or(0.);
//                         //     return GuiUnit::AspectPercent(n / 100.);
//                         // }
//                     });
//                     return GuiUnit::Percent(0.5);
//                 } else {
//                     if st.ends_with("%") {
//                         if st.ends_with("@%") {
//                             let n = st[0..st.len() - 2].parse::<f32>().unwrap_or(0.);
//                             return GuiUnit::AspectPercent(n / 100.);
//                         } else {
//                             let n = st.parse::<f32>().unwrap_or(0.);
//                             return GuiUnit::Percent(n / 100.);
//                         }
//                     } else if st.ends_with("@") {
//                         let n = st.parse::<u32>().unwrap_or(0);
//                         return GuiUnit::Pixel(n);
//                     } else {
//                         return GuiUnit::Percent(0.5);
//                     }
//                 }
//                 // match s {
//                 //     "center" => GuiUnit::Percent(0.5),
//                 //     "left" => GuiUnit::Percent(0.),
//                 //     "right" => GuiUnit::Percent(1.),
//                 // }
//             }
//             _ => GuiUnit::Pixel(0),
//         },
//         _ => GuiUnit::Pixel(0),
//     }
// }

fn table_hasher(table: mlua::Table) -> Vec<(String, ValueMap)> {
    let mut data = vec![];
    for it in table.pairs::<String, Value>() {
        if let Ok((key, val)) = it {
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

fn convert_quads(q: Vec<[f32; 3]>) -> (Vec<[f32; 3]>, Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<u32>) {
    let mut vec = vec![];
    let mut uv = vec![];
    let mut norms = vec![];
    let mut ind: Vec<u32> = vec![];
    let max = q.len() / 4;

    for i in 0..max {
        let i = i * 4;
        let v1 = q[i];
        let v2 = q[i + 1];
        let v3 = q[i + 2];
        let v4 = q[i + 3];

        // minus v1
        let v2 = [v2[0] - v1[0], v2[1] - v1[1], v2[2] - v1[2]];
        let v3 = [v3[0] - v1[0], v3[1] - v1[1], v3[2] - v1[2]];
        let v4 = [v4[0] - v1[0], v4[1] - v1[1], v4[2] - v1[2]];
        let v1 = [0., 0., 0.];

        let v1length =
            ((v2[0] - v1[0]).powi(2) + (v2[1] - v1[1]).powi(2) + (v2[2] - v1[2]).powi(2)).sqrt();
        let v2length =
            ((v3[0] - v2[0]).powi(2) + (v3[1] - v2[1]).powi(2) + (v3[2] - v2[2]).powi(2)).sqrt();

        let v3length =
            ((v4[0] - v3[0]).powi(2) + (v4[1] - v3[1]).powi(2) + (v4[2] - v3[2]).powi(2)).sqrt();

        let v4length =
            ((v1[0] - v4[0]).powi(2) + (v1[1] - v4[1]).powi(2) + (v1[2] - v4[2]).powi(2)).sqrt();

        // get direction of v1 to v2
        let v1dir = [v2[0] - v1[0], v2[1] - v1[1], v2[2] - v1[2]];

        // get direction of v2 to v3
        let v2dir = [v3[0] - v2[0], v3[1] - v2[1], v3[2] - v2[2]];
        // get direction of v3 to v4 but reverse
        let v3dir = [v4[0] - v3[0], v4[1] - v3[1], v4[2] - v3[2]];
        // get direction of v4 to v1 but reverse
        let v4dir = [v1[0] - v4[0], v1[1] - v4[1], v1[2] - v4[2]];

        // normalize
        let v1dir = if v1length > 0. {
            [
                v1dir[0] / v1length,
                v1dir[1] / v1length,
                v1dir[2] / v1length,
            ]
        } else {
            [0., 0., 0.]
        };

        let v2dir = if v2length > 0. {
            [
                v2dir[0] / v2length,
                v2dir[1] / v2length,
                v2dir[2] / v2length,
            ]
        } else {
            [0., 0., 0.]
        };

        let v3dir = if v3length > 0. {
            [
                v3dir[0] / v3length,
                v3dir[1] / v3length,
                v3dir[2] / v3length,
            ]
        } else {
            [0., 0., 0.]
        };
        let v4dir = if v4length > 0. {
            [
                v4dir[0] / v4length,
                v4dir[1] / v4length,
                v4dir[2] / v4length,
            ]
        } else {
            [0., 0., 0.]
        };
        let yaxis = [0., 1., 0.];
        //cross of v1dir and v2dir
        let quad_norm = [
            v1dir[1] * v2dir[2] - v1dir[2] * v2dir[1],
            v1dir[2] * v2dir[0] - v1dir[0] * v2dir[2],
            v1dir[0] * v2dir[1] - v1dir[1] * v2dir[0],
        ];
        println!("quad_norm {:?}", quad_norm);
        let rotmat = if quad_norm[1] > 0.95 {
            // some slight rounding errors but this is close enough and consistent
            [[1., 0., 0.], [0., 1., 0.], [0., 0., 1.]]
        } else {
            // dot product of quad_norm and yaxis
            // let norm_dot = quad_norm[0] * yaxis[0] + quad_norm[1] * yaxis[1] + quad_norm[2] * yaxis[2];
            // println!("norm_dot {:?}", norm_dot);

            //croos product of quad_norm and yaxis
            let norm_cross = [
                quad_norm[1] * yaxis[2] - quad_norm[2] * yaxis[1],
                quad_norm[2] * yaxis[0] - quad_norm[0] * yaxis[2],
                quad_norm[0] * yaxis[1] - quad_norm[1] * yaxis[0],
            ];

            let axis = [norm_cross[0], norm_cross[1], norm_cross[2]];

            let dot_prod =
                quad_norm[0] * yaxis[0] + quad_norm[1] * yaxis[1] + quad_norm[2] * yaxis[2];

            let cos_a = dot_prod;

            let k = 1. / (1.0 + cos_a);

            [
                [
                    (axis[0] * axis[0] * k) + cos_a,
                    (axis[1] * axis[0] * k) - axis[2],
                    (axis[2] * axis[0] * k) + axis[1],
                ],
                [
                    (axis[0] * axis[1] * k) + axis[2],
                    (axis[1] * axis[1] * k) + cos_a,
                    (axis[2] * axis[1] * k) - axis[0],
                ],
                [
                    (axis[0] * axis[2] * k) - axis[1],
                    (axis[1] * axis[2] * k) + axis[0],
                    (axis[2] * axis[2] * k) + cos_a,
                ],
            ]
        };

        //rotate quad_norm by rotmat
        let new_quad_norm = [
            rotmat[0][0] * quad_norm[0] + rotmat[0][1] * quad_norm[1] + rotmat[0][2] * quad_norm[2],
            rotmat[1][0] * quad_norm[0] + rotmat[1][1] * quad_norm[1] + rotmat[1][2] * quad_norm[2],
            rotmat[2][0] * quad_norm[0] + rotmat[2][1] * quad_norm[1] + rotmat[2][2] * quad_norm[2],
        ];
        println!("new_quad_norm {:?}", new_quad_norm); //should be [0,1,0]

        println!(
            "original v1dir {:?} v2dir {:?} v3dir {:?} v4dir {:?}",
            v1dir, v2dir, v3dir, v4dir
        );

        println!("rotmat {:?}", rotmat);
        //rotate v1dir
        let v1dir = [
            rotmat[0][0] * v1dir[0] + rotmat[0][1] * v1dir[1] + rotmat[0][2] * v1dir[2],
            rotmat[1][0] * v1dir[0] + rotmat[1][1] * v1dir[1] + rotmat[1][2] * v1dir[2],
            rotmat[2][0] * v1dir[0] + rotmat[2][1] * v1dir[1] + rotmat[2][2] * v1dir[2],
        ];

        //rotate v2dir
        let v2dir = [
            rotmat[0][0] * v2dir[0] + rotmat[0][1] * v2dir[1] + rotmat[0][2] * v2dir[2],
            rotmat[1][0] * v2dir[0] + rotmat[1][1] * v2dir[1] + rotmat[1][2] * v2dir[2],
            rotmat[2][0] * v2dir[0] + rotmat[2][1] * v2dir[1] + rotmat[2][2] * v2dir[2],
        ];

        //rotate v3dir
        let v3dir = [
            rotmat[0][0] * v3dir[0] + rotmat[0][1] * v3dir[1] + rotmat[0][2] * v3dir[2],
            rotmat[1][0] * v3dir[0] + rotmat[1][1] * v3dir[1] + rotmat[1][2] * v3dir[2],
            rotmat[2][0] * v3dir[0] + rotmat[2][1] * v3dir[1] + rotmat[2][2] * v3dir[2],
        ];

        //rotate v4dir
        let v4dir = [
            rotmat[0][0] * v4dir[0] + rotmat[0][1] * v4dir[1] + rotmat[0][2] * v4dir[2],
            rotmat[1][0] * v4dir[0] + rotmat[1][1] * v4dir[1] + rotmat[1][2] * v4dir[2],
            rotmat[2][0] * v4dir[0] + rotmat[2][1] * v4dir[1] + rotmat[2][2] * v4dir[2],
        ];

        let v1dirx = v1dir[0] * v1length;
        let v1diry = v1dir[2] * v1length;

        // get v3 aligned x and y distance
        let v3dirx = v3dir[0] * v3length;
        let v3diry = v3dir[2] * v3length;
        // get v2 aligned y and y distance
        let v2diry = v2dir[2] * v2length;
        let v2dirx = v2dir[0] * v2length;
        // get v4 aligned x and y distance
        // let v4diry = v4dir[2] * v4length;
        // let v4dirx = v4dir[0] * v4length;
        println!(
            "v1dir {:?} v2dir {:?} v3dir {:?} v4dir {:?}",
            v1dir, v2dir, v3dir, v4dir
        );
        println!(
            "v1length {:?} v2length {:?} v3length {:?} v4length {:?}",
            v1length, v2length, v3length, v4length
        );

        let n1 = [0., 0.];
        let n2 = [v1dirx, v1diry];
        let n3 = [n2[0] + v2dirx, n2[1] + v2diry];
        let n4 = [n3[0] + v3dirx, n3[1] + v3diry];
        println!("n1 {:?} n2 {:?} n3 {:?} n4 {:?}", n1, n2, n3, n4);

        let mut xs = [n1[0], n2[0], n3[0], n4[0]];
        let mut ys = [n1[1], n2[1], n3[1], n4[1]];

        let mut fail = false;
        xs.sort_by(|a, b| match a.partial_cmp(b) {
            Some(x) => x,
            None => {
                fail = true;
                Ordering::Less
            }
        });
        ys.sort_by(|a, b| match a.partial_cmp(b) {
            Some(x) => x,
            None => {
                fail = true;
                Ordering::Less
            }
        });
        if !fail {
            let minx = xs[0];
            let maxx = xs[3];
            let miny = ys[0];
            let maxy = ys[3];
            let rangex = maxx - minx;
            let rangey = maxy - miny;

            let mut u1 = [(-minx / rangex), (-miny / rangey)];
            let mut u2 = [(n2[0] - minx) / rangex, (n2[1] - miny) / rangey];
            let mut u3 = [(n3[0] - minx) / rangex, (n3[1] - miny) / rangey];
            let mut u4 = [(n4[0] - minx) / rangex, (n4[1] - miny) / rangey];
            // println!("u1 {:?} u2 {:?} u3 {:?} u4 {:?}", u1, u2, u3, u4);

            // scale to fit quads aspect but the larger axis is always 1
            if rangex < rangey {
                let scale = rangex / rangey;
                u1[0] *= scale;
                u2[0] *= scale;
                u3[0] *= scale;
                u4[0] *= scale;
            } else {
                let scale = rangey / rangex;
                u1[1] *= scale;
                u2[1] *= scale;
                u3[1] *= scale;
                u4[1] *= scale;
            };

            // let u1y = u1[1];
            // let u2y = u2[1];
            // u1[1] = u4[1];
            // u2[1] = u3[1];
            // u3[1] = u2y;
            // u4[1] = u1y;

            // println!("v1 {:?} v2 {:?} v3 {:?} v4 {:?}", v1, v2, v3, v4);
            // println!("2 [{:?}, {:?}] 3 [{:?} , {:?}] 4 [{:?} {:?}]", v2dirx, v2diry, v3dirx, v3diry, v4dirx, v4diry);
            // println!("n1 [{:?}, {:?}] n2 [{:?} , {:?}] n3 [{:?} {:?}] n4 [{:?} {:?}]", n1[0], n1[1], n2[0], n2[1], n3[0], n3[1], n4[0], n4[1]);
            // println!(
            //     "minx {:?} maxx {:?} miny {:?} maxy {:?}",
            //     minx, maxx, miny, maxy
            // );
            // println!("rangex {:?} rangey {:?}", rangex, rangey);
            // println!("f1 [{:?}, {:?}] f2 [{:?}, {:?}] f3 [{:?}, {:?}] f4 [{:?}, {:?}]", u1[0], u1[1], u2[0], u2[1], u3[0], u3[1], u4[0], u4[1]);
            // let v1dir=[v1[0] - v2[0], v1[1] - v2[1], v1[2] - v2[2]];

            uv.push(u1);
            uv.push(u2);
            uv.push(u3);
            uv.push(u4);
        } else {
            //default to basic ugly unwrap
            uv.push([0., 1.]);
            uv.push([1., 1.]);
            uv.push([1., 0.]);
            uv.push([0., 0.]);
        }

        norms.push(quad_norm);
        norms.push(quad_norm);
        norms.push(quad_norm);
        norms.push(quad_norm);

        vec.push(q[i]);
        vec.push(q[i + 1]);
        vec.push(q[i + 2]);
        vec.push(q[i + 3]);

        ind.push(i as u32);
        ind.push((i + 1) as u32);
        ind.push((i + 2) as u32);
        ind.push((i + 2) as u32);
        ind.push((i + 3) as u32);
        ind.push(i as u32);
    }
    (vec, norms, uv, ind)
}

fn make_err(s: &str) -> mlua::prelude::LuaError {
    return mlua::Error::RuntimeError(s.to_string());
}

/** Convert string command into easy to use hashmap */
fn getComHash(svec: Vec<&str>, paired: Vec<&str>) -> (Vec<String>, HashMap<String, String>) {
    let mut comhash = HashMap::new();
    let mut current = "";
    let mut regular = vec![];
    let mut pending = false;
    for s in svec {
        if pending {
            if s.starts_with("-") {
                comhash.insert(current.to_string(), "".to_string());
                current = &s[1..];
            } else {
                comhash.insert(current.to_string(), s.to_string());
                pending = false;
                current = "";
            }
        } else {
            if s.starts_with("-") {
                let c = &s[1..];
                if paired.contains(&c) {
                    current = c;
                    pending = true;
                } else {
                    comhash.insert(c.to_string(), "".to_string());
                }
            } else {
                regular.push(s.to_string());
            }
        }
    }
    (regular, comhash)
}

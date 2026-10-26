use crate::root::Core;
#[cfg(feature = "audio")]
use crate::sound::{Envelope, Instrument, Note, SoundCommand, WaveType};
#[cfg(feature = "audio")]
use crate::fx::{DriveShape, FilterKind};
#[cfg(feature = "audio")]
use crate::lua_define::SoundSender;
use crate::{
    bundle::{BundleMutations, BundleResources},
    error::P64Error,
    gui::GuiMorsel,
    log::LogType,
    lua_define::{LuaResponse, MainPacket},
    lua_ent::LuaEnt,
    lua_img::{dehex, LuaImg},
    model::{ModelPacket, TextureStyle},
    pad::Pad,
    pool::LocalPool,
    tile::Chunk,
    types::{GlobalMap, ValueMap},
    world::{TileCommand, TileResponse, World},
};
use colored::Colorize;

use image::RgbaImage;
use itertools::Itertools;
use pollster::{block_on, FutureExt};
use silt_lua::{
    gc_arena::{lock::RefLock, Gc, Mutation},
    lua::VM,
    userdata::{UserDataWrapper, WeakWrapper},
    value::Variadic,
    Compiler,
};

use parking_lot::Mutex;

#[cfg(feature = "puc_lua")]
use mlua::{
    AnyUserData, Error as ExtError, Lua, Table,
    Value::{self},
    Variadic,
};

#[cfg(feature = "silt")]
use silt_lua::prelude::{LuaError, Table, Value};

#[cfg(feature = "picc")]
use piccolo::{Context, Lua, Table};

#[cfg(feature = "silt")]
type ExtError = LuaError;
#[cfg(feature = "picc")]
type ExtError<'a> = LuaError<'a>;

use std::{
    cell::RefCell,
    cmp::Ordering,
    collections::HashMap,
    fmt::Display,
    rc::Rc,
    sync::mpsc::{sync_channel, Receiver, Sender, SyncSender},
};

macro_rules! lua_err {
    ($e:expr) => {
        if let Err(err) = $e {
            return Err(make_err(&err.to_string()));
        };
    };
}

static COM_LIST: [&str; 20] = [
    "new - creates a new game directory",
    "load - loads an app file",
    "pack - packs a directory into an app file",
    "unpack - unpacks an app file into a zip archive",
    "ls - show directory contents, relative to home",
    "show - open current app folder or file",
    "config - create config or show if already exists",
    // "reset - reset the app, same as",
    "exit - exit to boot menu",
    "reload - reloads game, or press Super + R",
    "atlas - dump texture atlas to png",
    "dev - toggle dev mode this session",
    "clear or cls - clear the console",
    "bg - set console background color",
    "find - loaded assets search",
    "stats - show stats",
    "notif - make a notification (debug)",
    "bundles - list active bundles",
    "help - ;)",
    "version - show engine and codex versions",
    "test",
];

/// Flush anything a just-torn-down bundle's Lua thread still had in flight to
/// the host, right after `hard_reset` — see `crate::drain_stray_messages`.
/// `catcher` is `None` on a caller that has no access to the receiver (e.g. a
/// wasm/worker path with no such queue); nothing to drain there.
fn drain_after_hard_reset(catcher: Option<&Receiver<MainPacket>>) {
    if let Some(catcher) = catcher {
        crate::drain_stray_messages(catcher);
    }
}

/// Private commands not reachable by lua code, but also works without lua
/// being loaded, returns false if not a valid command was entered and can
/// safely run through the lua runtime.
///
/// Supports shell-style piping: `left | right` runs `left`, captures
/// whatever it logged to the console (every command's real output already
/// goes through `core.loggy.log`, so this needs no per-command support) and
/// appends it as trailing arguments to `right`, and so on down the chain —
/// `pwd | copy` logs the app's directory, then copies exactly that to the
/// clipboard, and the same works for any other command's output.
pub fn run_con_sys(
    core: &mut Core,
    s: &str,
    catcher: Option<&Receiver<MainPacket>>,
) -> Result<bool, P64Error> {
    if !s.contains('|') {
        return run_con_sys_single(core, s, catcher);
    }
    let mut piped: Option<String> = None;
    let mut result = true;
    for stage in s.split('|') {
        let mut stage = stage.trim().to_string();
        if let Some(prev) = piped.take() {
            if !prev.is_empty() {
                stage.push(' ');
                stage.push_str(&prev);
            }
        }
        let mark = core.loggy.mark();
        result = run_con_sys_single(core, &stage, catcher)?;
        piped = Some(core.loggy.since(mark).join("\n"));
    }
    Ok(result)
}

fn run_con_sys_single(
    core: &mut Core,
    s: &str,
    catcher: Option<&Receiver<MainPacket>>,
) -> Result<bool, P64Error> {
    let bundle_id = core.bundle_manager.console_bundle_target;
    let main_bundle = core.bundle_manager.get_main_bundle();
    println!(" ({s})");
    if s.is_empty() {
        return Ok(false);
    }
    let segments = s.trim().split(" ").collect::<Vec<&str>>();

    match segments[0] {
        "die" => {
            // this chunk could probably be passed directly to lua core but being it's significance it felt important to pass into our pre-system check for commands
            core.loggy.log(
                LogType::Config,
                &format!("killing lua instance {bundle_id}"),
            );
            main_bundle.lua.die();
        }
        "bundles" => {
            core.loggy
                .log(LogType::Config, &core.bundle_manager.list_bundles());
        }
        // Spawn or dismiss an editing overlay: `overlay apps/mytool` / `overlay off`.
        //
        // Deliberately a *console* command and nothing else. Overlays are the
        // privileged surface — they reach the filesystem and edit another bundle — so
        // the app being edited must have no way to summon one. There is no Lua path
        // here to find, which is the point (see PLAN.md, overlay trust boundary).
        // Shorthand for the tool you reach for most: `edit` is `overlay apps/edit`,
        // and `edit off` closes it like any other overlay.
        "edit" => {
            let sub = if segments.len() > 1 { segments[1] } else { "" };
            if sub == "off" || sub == "close" {
                let n = close_overlays(core);
                core.loggy
                    .log(LogType::Config, &format!("closed {} overlay(s)", n));
            } else {
                match load_overlay(core, "apps/edit") {
                    Ok(id) => core
                        .loggy
                        .log(LogType::Config, &format!("editor up as bundle {}", id)),
                    Err(e) => core
                        .loggy
                        .log(LogType::ConfigError, &format!("editor failed: {}", e)),
                }
            }
        }
        "overlay" => {
            if segments.len() < 2 {
                core.loggy.log(
                    LogType::Config,
                    "usage: overlay <app path> | overlay off",
                );
            } else if segments[1] == "off" || segments[1] == "close" {
                let n = close_overlays(core);
                core.loggy
                    .log(LogType::Config, &format!("closed {} overlay(s)", n));
            } else {
                // Loaded as its own bundle, not a child of the app: reloading the app
                // shouldn't tear the editor down with it.
                match load_overlay(core, segments[1]) {
                    Ok(id) => {
                        core.loggy.log(
                            LogType::Config,
                            &format!("overlay '{}' up as bundle {}", segments[1], id),
                        );
                    }
                    Err(e) => core
                        .loggy
                        .log(LogType::ConfigError, &format!("overlay failed: {}", e)),
                }
            }
        }
        "pack" => {
            // new: path? name?
            // name, path, cartridge pic
            let (regular, com_hash) = if segments.len() > 1 {
                get_com_hash(segments[1..].to_vec(), ["o", "n", "i", "c"].to_vec())
            } else {
                (vec![], HashMap::new())
            };

            let current_game_dir = main_bundle.get_directory();
            crate::asset::pack(
                #[cfg(feature = "headed")]
                &mut core.tex_manager,
                &mut core.model_manager,
                &mut core.world,
                bundle_id,
                &main_bundle.lua,
                com_hash,
                regular,
                current_game_dir,
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
            )
            .block_on()?;
        }
        "superpack" => {
            core.loggy.log(
                LogType::Config,
                crate::asset::super_pack(if segments.len() > 1 {
                    segments[1]
                } else {
                    "game"
                })?,
            );
        }
        "unpack" => {
            if segments.len() > 1 {
                let name = segments[1];
                crate::file_util::unpack_and_save(
                    crate::file_util::get_file_buffer(&format!("./{}.game.zip", name))?,
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
            drain_after_hard_reset(catcher);
            load_app(
                core,
                if segments.len() > 1 {
                    // let targ = format!("{}.game.png", segments[1].to_string());
                    // let file = crate::zip_pal::get_file_buffer(&targ);
                    Some(segments[1])
                } else {
                    None
                },
                None,
                None,
                None,
            )?;
        }
        "exit" => {
            hard_reset(core);
            drain_after_hard_reset(catcher);
            load_empty(core);
        }
        "reset" => {
            hard_reset(core);
            drain_after_hard_reset(catcher);
        }
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
                Some(segments[1])
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
                        .log(LogType::ConfigError, &format!("read error: {er}"));
                }
            }
        }
        // Current app's directory — piping-friendly (see the `|` handling
        // wrapping this whole function): `pwd | copy` logs the path and puts
        // it straight on the clipboard.
        "pwd" => {
            core.loggy.log(
                LogType::Config,
                main_bundle.get_directory().unwrap_or(""),
            );
        }
        // Clipboard write. Takes its text as trailing args, or — the point of
        // this command existing — whatever the previous pipeline stage logged:
        // `pwd | copy`, `ls | copy`, `bundles | copy`, any command at all.
        "copy" => {
            let content = segments[1..].join(" ");
            if content.is_empty() {
                core.loggy
                    .log(LogType::ConfigError, "copy <text> (or pipe: `pwd | copy`)");
            } else {
                let _ = core
                    .pitcher
                    .send((bundle_id, MainCommmand::Copy(content.clone())));
                core.loggy
                    .log(LogType::Config, &format!("copied: {}", content));
            }
        }
        "ugh" => {
            if segments.len() > 1 {
                core.loggy
                    .log(LogType::Sys, &format!("heh, ya {}", segments[1]));
            } else {
                core.loggy.log(LogType::Sys, "heh, ya");
            }
        }
        "clear" => core.loggy.clear(),
        "cls" => core.loggy.clear(),
        "test" => {
            core.loggy.log(LogType::Config, "that test worked, yipee");
        }
        "notif" => {
            if segments.len() > 1 {
                core.gui.push_notif(segments[1]);
            } else {
                core.loggy.log(LogType::ConfigError, "notif <message>");
            }
        }
        "new" => {
            if segments.len() > 1 {
                let name = segments[1];

                eprintln!(
                    "[ new ] targeting bundle id {} (console_target {}) for help(true)",
                    main_bundle.id, bundle_id
                );
                let tout = main_bundle.lua.func("help(true)");
                match tout {
                    Ok(LuaResponse::Table(t)) => {
                        println!("ok in");
                        let mut mapper = HashMap::new();
                        for (k, c) in t.into_iter() {
                            let d: String = k.into();
                            let tup: (String, String) = c.into();
                            println!("### {}::{}::{}", d, tup.0, tup.1);
                            mapper.insert(d, tup);
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
                        crate::asset::make_directory(name, Some(&mapper), &mut core.loggy);
                        core.loggy
                            .log(LogType::Config, &format!("created directory {}", name));
                        hard_reset(core);
                        drain_after_hard_reset(catcher);
                        load_app(core, Some(name), None, None, None)?;
                    }
                    Ok(_t) => core.loggy.log(
                        LogType::ConfigError,
                        "Problem making directory (corrupt command table)",
                    ),
                    Err(e) => {
                        let s = format!("Problem making directory due to thrown error:{}", e);
                        core.loggy.log(LogType::ConfigError, &s);
                    }
                };
            } else {
                core.loggy.log(LogType::Config, "new <name>");
            }
        }
        "show" => {
            if segments.len() > 1 {
                let name = segments[1];
                crate::asset::open_dir(name);
            } else {
                match main_bundle.get_directory() {
                    Some(d) => {
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
            for c in COM_LIST {
                core.loggy.log(LogType::Config, c);
            }
        }
        "version" => {
            core.loggy.log(
                LogType::Config,
                &format!(
                    "Engine version {}, {}",
                    env!("CARGO_PKG_VERSION"),
                    crate::asset::get_codex_version_string().split_at(3).1
                ),
            );
        }
        &_ => return Ok(false),
    }
    Ok(true)
}

/// Parse the trailing `cfg` argument shared by `instr`/`smpl`. Returns the
/// ADSR envelope plus the command-specific scalar (`scalar_key` — "wid" for
/// pulse duty, "base" for sample pitch). `cfg` may be:
/// - a table `{ <scalar_key>=, atk=, dec=, sus=, rel= }` (any subset), or
/// - a bare number (legacy: the scalar only — `instr(id,'pulse',0.25)`), or
/// - nil/absent (defaults).
#[cfg(feature = "audio")]
fn parse_sound_cfg(cfg: Option<&Value>, scalar_key: &str) -> (Envelope, Option<f32>) {
    let mut env = Envelope::default();
    let mut scalar = None;
    match cfg {
        // legacy bare number = the type scalar only (e.g. instr(id,'pulse',0.25))
        Some(v @ (Value::Number(_) | Value::Integer(_))) => scalar = Some(v.into()),
        Some(Value::Table(t)) => {
            let tb = t.borrow();
            if let Some(v) = tb.get(scalar_key) {
                scalar = Some(v.into());
            }
            if let Some(v) = tb.get("atk") {
                env.attack = v.into();
            }
            if let Some(v) = tb.get("dec") {
                env.decay = v.into();
            }
            if let Some(v) = tb.get("sus") {
                env.sustain = v.into();
            }
            if let Some(v) = tb.get("rel") {
                env.release = v.into();
            }
        }
        _ => {}
    }
    (env, scalar)
}

thread_local! {
    /// Every command `init_lua_sys` last registered on this thread, as (name,
    /// (description, LSP stub)): what `ignore.lua` is made of (`command_docs`).
    static COMMAND_DOCS: RefCell<Vec<(String, (String, String))>> = RefCell::new(Vec::new());
}

/// Every Lua command with its description and LSP stub, for writing
/// `ignore.lua` without a running game: builds a throwaway VM, wired to
/// nothing, just to register them.
#[cfg(all(feature = "silt", not(target_arch = "wasm32")))]
pub fn command_docs() -> Result<HashMap<String, (String, String)>, P64Error> {
    let size = [1, 1];
    let letters = crate::gui::Gui::letter_init(&mut crate::log::Loggy::new());
    let gui = Rc::new(RefCell::new(GuiMorsel::new(letters, size)));
    let (pitcher, _catcher) = std::sync::mpsc::channel::<MainPacket>();
    let (loggy, _loggy_rx) = std::sync::mpsc::channel::<(LogType, String)>();
    let (world, _world_rx) = std::sync::mpsc::channel::<(TileCommand, SyncSender<TileResponse>)>();
    #[cfg(feature = "audio")]
    let (singer, _singer_rx) = std::sync::mpsc::channel::<SoundCommand>();

    let mut lua = silt_lua::Lua::new_with_standard();
    lua.enter::<_, Result<(), P64Error>>(|vm, mc| {
        init_lua_sys(
            vm,
            mc,
            0,
            false,
            pitcher.clone(),
            world.clone(),
            Rc::clone(&gui),
            #[cfg(feature = "audio")]
            singer.clone(),
            Rc::new(RefCell::new([false; 256])),
            Rc::new(RefCell::new([false; 256])),
            Rc::new(RefCell::new([0f32; 13])),
            Rc::new(RefCell::new(Pad::new())),
            Rc::new(Mutex::new(2u64)),
            loggy.clone(),
            LocalPool::new(),
        )
    })?;
    Ok(COMMAND_DOCS.with(|d| d.borrow().iter().cloned().collect()))
}

pub fn init_lua_sys<'a, 'gc>(
    #[cfg(feature = "picc")] ctx: &Context<'gc>,
    #[cfg(feature = "silt")] vm_init: &mut VM<'gc>,
    // lua_globals: &Table<'gc>,
    mc_in: &Mutation<'gc>,
    // executor: &Executor<'gc>,
    bundle_id: u8,
    // Is this bundle an engine-marked overlay? Decides whether the privileged `app.*`
    // table gets built at all, so it has to be known before the VM runs a line —
    // which is why overlays are marked as they load rather than afterwards.
    is_overlay: bool,
    main_pitcher: Sender<MainPacket>,
    world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
    gui_in: Rc<RefCell<GuiMorsel>>,
    // main_rast: Rc<RefCell<LuaImg>>,
    // sky_rast: Rc<RefCell<LuaImg>>,
    #[cfg(feature = "audio")] singer: SoundSender,
    keys: Rc<RefCell<[bool; 256]>>,
    diff_keys: Rc<RefCell<[bool; 256]>>,
    mice: Rc<RefCell<[f32; 13]>>,
    gamepad: Rc<RefCell<Pad>>,
    ent_counter: Rc<Mutex<u64>>,
    loggy: Sender<(LogType, String)>,
    shared: LocalPool,
) -> Result<(), P64Error>
// where N: 
// #[cfg(feature = "online_capable")]
// Option<(Sender<MovePacket>, Receiver<MovePacket>)> 
// #[cfg(not(feature = "online_capable"))]
// Option<bool>
{
    println!("init lua sys");

    // let c = *ctx;
    // let v = Value::String(piccolo::String::from_static(
    //     ctx.deref(),
    //     "default function",
    // ));

    // let default_func = native_function(ctx, |c, _, mut stack| {
    //     let s = c.intern_static("default function".as_bytes());
    //     stack.push_front(Value::String(s));
    //     // let v = Value::String(piccolo::String::from_static(ctx.deref())
    //     // stack.push_front(v);
    //     Ok(CallbackReturn::Return)
    //     // Ok(piccolo::CallbackReturn::Sequence(AnySequence::from(
    //     //     vec![Value::String("default function".to_string())]
    //     // )))
    // });

    #[cfg(feature = "puc_lua")]
    res(
        "_default_func",
        lua_globals.set("_default_func", default_func),
        &loggy,
    );
    // #[cfg(feature = "silt")]
    // lua_globals.set("_default_func", default_func);
    //
    let mut compiler = Compiler::new();
    let gui = gui_in.clone();

    let mut command_map: Vec<(String, (String, String))> = vec![];
    let mut io = vm_init.raw_table();

    // let c = *vm;
    let mut globals = vm_init.globals.borrow_mut(mc_in);
    globals.set("pi", std::f64::consts::PI);
    globals.set("tau", std::f64::consts::PI * 2.0);
    drop(globals);

    // vm_init.create_userdata(mc, data)
    // globals.set( "gui", main_rast);
    // globals.set( "sky", sky_rast);

    // lua_ctx.set_warning_function(|a, b, f| {
    //     log(format!("hi {:?}", b));
    //     Ok(())
    // });

    #[macro_export]
    macro_rules! lua {
        ($name:expr,$closure:expr,$desc:expr,$exam:expr) => {
            lua_lib!($name, $closure, $desc, $exam);
        };
    }

    #[macro_export]
    macro_rules! lua_lib {
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
            // #[cfg(feature = "puc_lua")]
            // res(
            //     $name,
            //     $lib.set($name, lua_ctx.create_function($closure).unwrap()),
            //     &loggy,
            // );
            #[cfg(feature = "silt")]
            vm_init.register_native_function(mc_in, $name, $closure);
        };

        ($name:expr,$closure:expr,$desc:expr,$exam:expr, $lib:expr) => {
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
            // #[cfg(feature = "puc_lua")]
            // res(
            //     $name,
            //     $lib.set($name, lua_ctx.create_function($closure).unwrap()),
            //     &loggy,
            // );
            #[cfg(feature = "silt")]
            vm_init.register_native_function_to(mc_in, $lib, $name, $closure);
        };
    }

    let aux_loggy = loggy.clone();
    lua!(
        "cout",
        move |_, _, args: Variadic| {
            println!("cout: {:?}", args.len());
            #[cfg(feature = "headed")]
            {
                aux_loggy.send((LogType::Lua, args.iter().join(", ")));
            }
            #[cfg(not(feature = "headed"))]
            {
                println!("{}", args.iter().join(", "));
            }
            Ok(())
        },
        "Prints string to console",
        "
---@param ... string|number|integer|boolean|image|entity|connection|nil
function cout(...) end"
    );

    let sender = world_sender.clone();
    lua!(
        "tile",
        move |_, _, (t, x, y, z, r): (Value, i32, i32, i32, Option<u8>)| {
            let tile = match t {
                Value::String(s) => s,
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
        move |_,_, (x, y, z): (Option<i32>, Option<i32>, Option<i32>)| {
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

    let pitcher = main_pitcher.clone();
    lua!(
        "tileset",
        move |_, _, (name, tiles, opts): (String, Option<Value>, Option<Value>)| {
            let tiles = match tiles {
                Some(Value::Table(t)) => t
                    .borrow()
                    .iter()
                    .filter_map(|(_, v)| match v {
                        Value::String(s) => Some(s.to_owned()),
                        _ => None,
                    })
                    .collect(),
                Some(Value::String(s)) => vec![s],
                _ => vec![],
            };
            let mut look = crate::tileset::Look::Block;
            let mut pieces = vec![];
            if let Some(Value::Table(t)) = opts {
                let t = t.borrow();
                if let Some(Value::String(s)) = t.get("look") {
                    look = crate::tileset::Look::parse(s);
                }
                if let Some(Value::Table(p)) = t.get("pieces") {
                    for (k, v) in p.borrow().iter() {
                        let case = match k {
                            Value::String(s) => crate::tileset::parse_case(&s),
                            Value::Integer(i) if (0..256).contains(&i) => Some(i as u8),
                            _ => None,
                        };
                        match (case, v) {
                            (Some(c), Value::String(m)) => pieces.push((c, m.to_owned())),
                            _ => {
                                return Err(make_err(&format!(
                                    "tileset {}: a piece is [case] = \"model\", case as 8 corner bits like \"1111 0000\" or 0..255",
                                    name
                                )))
                            }
                        }
                    }
                }
            }
            lua_err!(pitcher.send((
                bundle_id,
                MainCommmand::Tileset(crate::tileset::TilesetDef { name, tiles, look, pieces })
            )));
            Ok(())
        },
        "Mesh tiles together on the dual grid: blocky, smooth, or your own model pieces",
        "
---@param name string
---@param tiles string[]|nil
---@param opts {look?: 'block'|'smooth', pieces?: table<string|integer, string>}?
function tileset(name, tiles, opts) end"
    );

    // MARK
    // BLUE TODO this function is expensive? if called twice in one cycle it ruins key press checks??
    let sender = world_sender.clone();
    lua!(
        "istile",
        move |_, _, (x, y, z): (i32, i32, i32)| { Ok(World::is_tile(&sender, x, y, z)) },
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
        move |_, _, (x, y, z): (i32, i32, i32)| {
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
        move |l, mc, (t, x, y, z, dx, dy, dz): (String, i32, i32, i32, i32, i32, i32)| {
            let tt = if t.len() == 0 { None } else { Some(t) };
            match World::first_tile(&sender, tt, x, y, z, dx, dy, dz, 100) {
                Some(v) => Ok(l.table_from_array(mc, vec![v[0], v[1], v[2]])),
                None => Ok(l.new_table(mc)),
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
        move |_, _, (name, items, speed): (String, Vec<String>, Option<u32>)| {
            // println!("we have anims {:?}", items);
            let anim_speed = match speed {
                Some(s) => s,
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
        move |_, _, (key, volatile): (String, Option<bool>)| {
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
        move |_, _, _: ()| {
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
        move |vm, mc, (): ()| {
            let mut t = vm.raw_table();
            let m = mice.borrow();
            t.set("x", m[0]);
            t.set("y", m[1]);
            t.set("dx", m[2]);
            t.set("dy", m[3]);
            t.set("px", m[4]);
            t.set("py", m[5]);

            // t.set("z",m[2])?;
            t.set("m1", m[6] > 0.);
            t.set("m2", m[7] > 0.);
            t.set("m3", m[8] > 0.);
            t.set("scroll", m[9]);

            t.set("vx", m[10]);
            t.set("vy", m[11]);
            t.set("vz", m[12]);

            drop(m);
            // Return an explicit `Value::Table` rather than leaning on the
            // implicit `ToLua for Table` boundary conversion.
            Ok(vm.wrap_table(mc, t))
        },
        " Get mouse position, delta, button states, and unprojected vector",
        "
---@return mouse
function mus() end"
    );

    let gam = Rc::clone(&gamepad);
    lua!(
        "btn",
        move |_, _, button: String| { Ok(gam.borrow().check(button) != 0.) },
        "Check if gamepad button is held down",
        "
---@param button string
---@return boolean
function btn(button) end"
    );

    lua!(
        "abtn",
        move |_, _, button: String| { Ok(gamepad.borrow().check(button)) },
        "Check how much a gamepad is pressed, axis gives value between -1 and 1",
        "
---@param button string
---@return number
function abtn(button) end"
    );

    let pitcher = main_pitcher.clone();
    let ent_counter2 = ent_counter.clone();
    lua!(
        "make",
        move |vm,
              mc,
              (asset, x, y, z, s): (
            Option<String>,
            Option<f64>,
            Option<f64>,
            Option<f64>,
            Option<f64>
        )| {
            let id = *ent_counter.lock();
            *ent_counter.lock() += 1;

            let ent = crate::lua_ent::LuaEnt::new(
                id,
                asset.unwrap_or("plane".to_string()),
                x.unwrap_or(0.),
                y.unwrap_or(0.),
                z.unwrap_or(0.),
                s.unwrap_or(1.),
            );
            let wrapper = vm.create_userdata_raw(mc, ent);
            let wrapper_clone = wrapper.clone();
            // let wrapped = Arc::new(std::sync::Mutex::new(ent));

            // match pitcher.send(MainCommmand::Spawn(asset, x, y, z, s.unwrap_or(1.), 1, tx)) {

            match pitcher.send((bundle_id, MainCommmand::Spawn(wrapper_clone))) {
                Ok(_) => {}
                Err(_) => return Err(static_err("Unable to create entity")),
            }
            // AnyUserData::
            // AnyUserData::new(lua_ctx, wrapped).to_lua_err()?;

            // Ok(match rx.recv() {
            //     Ok(mut e) => e.remove(0),
            //     Err(e) => Arc::new(std::sync::Mutex::new(LuaEnt::empty())),
            // })
            let val = Value::UserData(Gc::new(mc, RefLock::new(wrapper)));
            Ok(val)
        },
        "Spawn an entity from an asset",
        "
---@param asset string?
---@param x number?
---@param y number?
---@param z number?
---@param scale number?
---@return entity
function make(asset, x, y, z, scale) end"
    );

    // single usage method for entity duplication only means it's not listed
    let pitcher = main_pitcher.clone();
    vm_init.register_native_function(mc_in, "_make", move |v, mc, ud: Value| {
        // lent: Arc<std::sync::Mutex<LuaEnt>>
        // MARK required 1
        if let Value::UserData(lent) = ud {
            let id = *ent_counter2.lock();
            let lb = lent.borrow();
            if lb.is_type::<LuaEnt>() {
                let l2 = lb.clone();
                *ent_counter2.lock() += 1;
                match pitcher.send((bundle_id, MainCommmand::Spawn(l2))) {
                    Ok(_) => {}
                    Err(_) => return Err(context_err("Unable to create entity")),
                };
            } else {
                return Err(context_err("Invalid entity passed to make"));
            }
        } else {
            return Err(context_err("Invalid entity passed to make"));
        }

        Ok(Value::Nil)
    });


    let pitcher = main_pitcher.clone();
    lua!(
        "lot",
        move |_, _, pair: (Value, Value)| {
            if let (Value::UserData(parent), Value::UserData(child)) = pair {
                let id1 = parent
                    .borrow()
                    .downcast_get::<LuaEnt, _, _>(|p| Ok(p.get_id()));
                let id2 = child
                    .borrow()
                    .downcast_get::<LuaEnt, _, _>(|c| Ok(c.get_id()));
                if let (Ok(parent_id), Ok(child_id)) = (id1, id2) {
                    // Not waited on: it rides the same queue as the child's
                    // Spawn, so it lands after it, and the parent is set on the
                    // shared entity before anything draws it. Waiting cost a
                    // frame per call (a model editor parenting a handle per
                    // vertex stalled for seconds).
                    if pitcher.send((bundle_id, MainCommmand::Group(parent_id, child_id))).is_err() {
                        return Err(static_err("Unable to group entity"));
                    }
                } else {
                    return Err(static_err("Could not retrieve parent or child id to group"));
                }
            }

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
        move |_, mc, ent: Value| {
            if let Value::UserData(mut r) = ent {
                r.borrow_mut(mc)
                    .downcast_mut::<LuaEnt, _, _>(|ud| Ok(ud.kill()));
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
        move |_, _, (): ()| {
            // println!("hit reset");
            match pitcher.send((bundle_id, MainCommmand::Reload())) {
                Ok(_) => {}
                Err(_er) => {}
            }
            Ok(())
        },
        "Resets lua context and reloads all assets and scripts fresh",
        "
function reload() end"
    );

    let pitcher = main_pitcher.clone();
    #[cfg(feature = "audio")]
    let attr_singer = singer.clone();
    lua!(
        "attr",
        move |lu, mc, table: Option<Value>| {
            match table {
                Some(Value::Table(t)) => {
                    // Audio: a `lanes = {4,3,5}` array sets per-channel polyphony
                    // (entry i → channel i-1). Extracted here and pushed to the
                    // synth; the rest of the table is app-state globals as usual.
                    #[cfg(feature = "audio")]
                    {
                        if let Some(Value::Table(lanes_t)) = t.borrow().get("lanes") {
                            let lt = lanes_t.borrow();
                            let mut counts = Vec::new();
                            let mut i = 1;
                            while let Some(v) = lt.getn(i) {
                                let n: f32 = v.into();
                                counts.push(n.max(1.0) as usize);
                                i += 1;
                            }
                            let _ = attr_singer.send(SoundCommand::SetLanes(counts));
                        }
                    }
                    let hash = table_hasher(&t.borrow());
                    lua_err!(pitcher.send((bundle_id, MainCommmand::Globals(hash))));

                    Ok(Value::Nil)
                }
                _ => {
                    let (tx, rx) = std::sync::mpsc::sync_channel::<GlobalMap>(0);
                    lua_err!(pitcher.send((bundle_id, MainCommmand::GetGlobal(tx))));
                    match rx.recv() {
                        Ok(arr) => {
                            let mut t = lu.raw_table();

                            arr.convert(lu, mc, &mut t);
                            Ok(lu.wrap_table(mc, t))
                        }
                        Err(_) => Ok(Value::Nil),
                    }
                }
            }
        },
        "Set app state params or get attributes if no args",
        "
---@param attributes attributes?
function attr(attributes) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "cam",
        move |_, _, table_val: Value| {
            if let Value::Table(t) = table_val {
                let table = t.borrow();
                let pos = match table.get("pos") {
                    Some(v) => match v {
                        Value::Table(tbl) => {
                            let t = tbl.borrow();
                            let x = t.getn(1).unwrap_or(&Value::Nil).into();
                            let y = t.getn(2).unwrap_or(&Value::Nil).into();
                            let z = t.getn(3).unwrap_or(&Value::Nil).into();
                            Some(glam::vec3(x, y, z))
                        }
                        _ => None,
                    },
                    _ => None,
                };
                let rot = match table.get("rot") {
                    Some(v) => match v {
                        Value::Table(tbl) => {
                            let t = tbl.borrow();
                            let x = t.getn(1).unwrap_or(&Value::Nil).into();
                            let y = t.getn(2).unwrap_or(&Value::Nil).into();
                            Some(glam::vec2(x, y))
                        }
                        _ => None,
                    },
                    _ => None,
                };

                // a half-height in world units, or false / 0 for perspective
                let ortho = match table.get("ortho") {
                    Some(Value::Bool(_)) => Some(0.),
                    Some(v) => {
                        let h: f32 = v.into();
                        Some(h.max(0.))
                    }
                    None => None,
                };

                lua_err!(pitcher.send((bundle_id, MainCommmand::Cam(pos, rot, ortho))));
            }

            Ok(())
        },
        "Set the camera position and/or rotation",
        "
---@param params cam_params
function cam(params) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "lum",
        move |_, _, table_val: Value| {
            if let Value::Table(t) = table_val {
                let table = t.borrow();
                // A raw xyz vector table (not a colour) — shared by `dir`/`pos`.
                let vec3_of = |key: &str| match table.get(key) {
                    Some(Value::Table(tbl)) => {
                        let t = tbl.borrow();
                        Some(glam::vec3(
                            t.getn(1).unwrap_or(&Value::Nil).into(),
                            t.getn(2).unwrap_or(&Value::Nil).into(),
                            t.getn(3).unwrap_or(&Value::Nil).into(),
                        ))
                    }
                    _ => None,
                };
                let dir = vec3_of("dir");
                let pos = vec3_of("pos");
                // Colours accept the usual forms: hex string ('ff0'), rgb table
                // (0..1 or 0..255) — via the shared get_color helper.
                let color_of = |key: &str| {
                    table.get(key).map(|v| {
                        let c = crate::lua_img::get_color(v.clone());
                        glam::vec3(c.x, c.y, c.z)
                    })
                };
                let color = color_of("color");
                let sky = color_of("sky");
                let ground = color_of("ground");
                let ambient = match table.get("ambient") {
                    Some(v) => Some(v.into()),
                    _ => None,
                };
                let shape = match table.get("shape") {
                    Some(Value::String(s)) => match s.to_lowercase().as_str() {
                        "sun" => Some(0u8),
                        "cone" => Some(1u8),
                        "sphere" => Some(2u8),
                        _ => return Err(static_err("lum: unknown shape (want sun/cone/sphere)")),
                    },
                    _ => None,
                };
                let range = match table.get("range") {
                    Some(v) => Some(v.into()),
                    _ => None,
                };
                let angle = match table.get("angle") {
                    Some(v) => Some(v.into()),
                    _ => None,
                };
                lua_err!(pitcher.send((
                    bundle_id,
                    MainCommmand::Light(dir, color, ambient, sky, ground, shape, pos, range, angle)
                )));
            }
            Ok(())
        },
        "Set the one light: shape (\"sun\"/\"cone\"/\"sphere\", default sun), dir/pos (xyz), color (rgb 0..1), ambient (0..1), sky/ground (hemisphere, sun only), range, angle (cone half-angle, radians)",
        "
---@param params lum_params
function lum(params) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "fog",
        move |_, _, table_val: Value| {
            if let Value::Table(t) = table_val {
                let table = t.borrow();
                // Colour accepts hex string ('9bd') or rgb table via get_color.
                let (r, g, b) = match table.get("color") {
                    Some(v) => {
                        let c = crate::lua_img::get_color(v.clone());
                        (c.x, c.y, c.z)
                    }
                    _ => (0., 0., 0.),
                };
                // `dist` is the far distance where geometry is fully fogged; 0 = off.
                let dist: f32 = match table.get("dist") {
                    Some(v) => v.into(),
                    _ => 0.,
                };
                lua_err!(pitcher.send((bundle_id, MainCommmand::Fog(glam::vec4(r, g, b, dist)))));
            }
            Ok(())
        },
        "Set distance fog: color (rgb 0..1) blended in by dist (far, world units; 0 = off)",
        "
---@param params fog_params
function fog(params) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "mgrab",
        move |_, _, on: Option<bool>| {
            // Ask the host to capture the mouse (native: cursor grab; web:
            // pointer-lock on the next canvas click). While grabbed, mus() dx/dy
            // report raw movement for FPS-style look. Defaults to true.
            lua_err!(pitcher.send((bundle_id, MainCommmand::MouseGrab(on.unwrap_or(true)))));
            Ok(())
        },
        "Grab (capture) the mouse for relative look, or release it with mgrab(false)",
        "
---@param on boolean?
function mgrab(on) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "shdw",
        move |_, _, arg: Value| {
            // Off by default: the shadow pass only runs once a script opts in
            // — same "zero behavior change" discipline as chip()/mon().
            // shdw() / shdw(bool) toggle; shdw{on=, size=, range=} also tunes
            // the map, leaving any field it omits as it was.
            let cmd = match arg {
                Value::Bool(on) => MainCommmand::Shdw(on, None, None),
                Value::Table(t) => {
                    let table = t.borrow();
                    let on = match table.get("on") {
                        Some(Value::Bool(b)) => *b,
                        _ => true,
                    };
                    let size: Option<f32> = table.get("size").map(|v| v.into());
                    let range: Option<f32> = table.get("range").map(|v| v.into());
                    MainCommmand::Shdw(on, size, range)
                }
                _ => MainCommmand::Shdw(true, None, None),
            };
            lua_err!(pitcher.send((bundle_id, cmd)));
            Ok(())
        },
        "Enable the shadow map (from the current lum light), disable it with shdw(false), or tune it with shdw{size=texels, range=tiles}",
        "
---@class shdw_params
---@field on? boolean
---@field size? integer shadow map edge length in texels (default 128)
---@field range? number sun shadow half-width around the camera, in tiles (default 25)

---@param on boolean|shdw_params?
function shdw(on) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "gour",
        move |_, _, on: Option<bool>| {
            // Off by default: today's per-fragment "smooth" shading stays
            // the default — see guide/gour.md.
            lua_err!(pitcher.send((bundle_id, MainCommmand::Gour(on.unwrap_or(true)))));
            Ok(())
        },
        "Enable Gouraud (per-vertex) shading, or disable it with gour(false) to go back to per-fragment",
        "
---@param on boolean?
function gour(on) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "chip",
        move |_, _, code: Option<String>| {
            match code {
                Some(c) => {
                    let id = match c.to_lowercase().as_str() {
                        "r00" => 0u8,
                        "r43" => 1u8,
                        "r30" => 2u8,
                        _ => return Err(static_err("chip: unknown code (want r00/r43/r30)")),
                    };
                    lua_err!(pitcher.send((bundle_id, MainCommmand::SetChip(id))));
                    Ok(Value::Nil)
                }
                None => {
                    let (tx, rx) = sync_channel::<u8>(0);
                    lua_err!(pitcher.send((bundle_id, MainCommmand::GetChip(tx))));
                    let name = match rx.recv().unwrap_or(0) {
                        1 => "r43",
                        2 => "r30",
                        _ => "r00",
                    };
                    Ok(Value::String(name.to_string()))
                }
            }
        },
        "Get or set the graphics chip preset: r00 (modern), r43 (N64-ish), r30 (PS1-ish)",
        "
---@param code string?
---@return string?
function chip(code) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "mon",
        move |_, _, code: Option<String>| {
            match code {
                Some(c) => {
                    let id = match c.to_lowercase().as_str() {
                        "lcd" => 0u8,
                        "slot" => 1u8,
                        "grille" => 2u8,
                        _ => return Err(static_err("mon: unknown code (want lcd/slot/grille)")),
                    };
                    lua_err!(pitcher.send((bundle_id, MainCommmand::SetMonitor(id))));
                    Ok(Value::Nil)
                }
                None => {
                    let (tx, rx) = sync_channel::<u8>(0);
                    lua_err!(pitcher.send((bundle_id, MainCommmand::GetMonitor(tx))));
                    let name = match rx.recv().unwrap_or(0) {
                        1 => "slot",
                        2 => "grille",
                        _ => "lcd",
                    };
                    Ok(Value::String(name.to_string()))
                }
            }
        },
        "Get or set the display monitor preset: lcd (modern flat panel), slot (slot-mask CRT), grille (aperture-grille CRT)",
        "
---@param code string?
---@return string?
function mon(code) end"
    );

    // The collision system's queries — see guide/hit.md. `hit.pair(a, b)` is
    // a function, not a colon method (`ent1:hit(ent2)` as originally asked):
    // silt-lua's native-call plumbing (`NativeReturn`) isn't reachable from
    // outside the crate, so a `LuaEnt` userdata method has no way to bridge
    // to a channel-backed native and get its return value back — only fire-
    // and-forget (the `copy`->`_make` pattern) works there. A plain function
    // has full pitcher access like any other native, so that's the shape.
    let mut hit = vm_init.raw_table();

    let pitcher = main_pitcher.clone();
    lua_lib!(
        "all",
        move |vm, mc, _: ()| {
            let (tx, rx) = sync_channel::<Vec<(u64, u64, f32, f32, f32, f32)>>(0);
            lua_err!(pitcher.send((bundle_id, MainCommmand::HitAll(tx))));
            let hits = rx.recv().unwrap_or_default();
            let out: Vec<Value> = hits
                .into_iter()
                .map(|(a, b, nx, ny, nz, depth)| {
                    let mut t = vm.raw_table();
                    t.set("a", a as f64);
                    t.set("b", b as f64);
                    t.set(
                        "normal",
                        vm.table_from_array(mc, [nx as f64, ny as f64, nz as f64]),
                    );
                    t.set("depth", depth as f64);
                    vm.wrap_table(mc, t)
                })
                .collect();
            Ok(vm.table_from_array(mc, out))
        },
        "Every currently-overlapping ent-vs-ent pair in this bundle: array of {a=id, b=id, normal={x,y,z}, depth=n}",
        "
---@return table[]
function hit.all() end",
        &mut hit
    );

    let pitcher = main_pitcher.clone();
    lua_lib!(
        "cell",
        move |vm, mc, id: u64| {
            let (tx, rx) = sync_channel::<Vec<(i32, i32, i32, f32, f32, f32, f32)>>(0);
            lua_err!(pitcher.send((bundle_id, MainCommmand::HitCell(id, tx))));
            let hits = rx.recv().unwrap_or_default();
            let out: Vec<Value> = hits
                .into_iter()
                .map(|(ix, iy, iz, nx, ny, nz, depth)| {
                    let mut t = vm.raw_table();
                    t.set(
                        "tile",
                        vm.table_from_array(mc, [ix as f64, iy as f64, iz as f64]),
                    );
                    t.set(
                        "normal",
                        vm.table_from_array(mc, [nx as f64, ny as f64, nz as f64]),
                    );
                    t.set("depth", depth as f64);
                    vm.wrap_table(mc, t)
                })
                .collect();
            Ok(vm.table_from_array(mc, out))
        },
        "Every solid tile entity `id`'s collider currently overlaps: array of {tile={x,y,z}, normal={x,y,z}, depth=n}",
        "
---@param id integer
---@return table[]
function hit.cell(id) end",
        &mut hit
    );

    let pitcher = main_pitcher.clone();
    lua_lib!(
        "grid",
        move |vm, mc, id: u64| {
            let (tx, rx) = sync_channel::<Vec<(u64, i32, i32, i32, f32, f32, f32, f32)>>(0);
            lua_err!(pitcher.send((bundle_id, MainCommmand::HitGrid(id, tx))));
            let hits = rx.recv().unwrap_or_default();
            let out: Vec<Value> = hits
                .into_iter()
                .map(|(owner, ix, iy, iz, nx, ny, nz, depth)| {
                    let mut t = vm.raw_table();
                    t.set("owner", owner as f64);
                    t.set(
                        "tile",
                        vm.table_from_array(mc, [ix as f64, iy as f64, iz as f64]),
                    );
                    t.set(
                        "normal",
                        vm.table_from_array(mc, [nx as f64, ny as f64, nz as f64]),
                    );
                    t.set("depth", depth as f64);
                    vm.wrap_table(mc, t)
                })
                .collect();
            Ok(vm.table_from_array(mc, out))
        },
        "Every hit between entity `id`'s collider and any other entity's tile grid: array of {owner=id, tile={x,y,z}, normal={x,y,z}, depth=n}",
        "
---@param id integer
---@return table[]
function hit.grid(id) end",
        &mut hit
    );

    let pitcher = main_pitcher.clone();
    lua_lib!(
        "pair",
        move |vm, mc, (ud_a, ud_b): (Value, Value)| {
            let extract = |ud: Value| -> Option<UserDataWrapper> {
                if let Value::UserData(lent) = ud {
                    let lb = lent.borrow();
                    if lb.is_type::<LuaEnt>() {
                        return Some(lb.clone());
                    }
                }
                None
            };
            let a = match extract(ud_a) {
                Some(w) => w,
                None => return Err(context_err("hit.pair expects two entities")),
            };
            let b = match extract(ud_b) {
                Some(w) => w,
                None => return Err(context_err("hit.pair expects two entities")),
            };
            let (tx, rx) = sync_channel::<Option<(f32, f32, f32, f32)>>(0);
            lua_err!(pitcher.send((bundle_id, MainCommmand::HitPair(a, b, tx))));
            match rx.recv() {
                Ok(Some((nx, ny, nz, depth))) => {
                    let mut t = vm.raw_table();
                    t.set(
                        "normal",
                        vm.table_from_array(mc, [nx as f64, ny as f64, nz as f64]),
                    );
                    t.set("depth", depth as f64);
                    Ok(vm.wrap_table(mc, t))
                }
                _ => Ok(Value::Nil),
            }
        },
        "Pairwise collision test between two entities: nil or {normal={x,y,z}, depth=n}",
        "
---@param a entity
---@param b entity
---@return table?
function hit.pair(a, b) end",
        &mut hit
    );

    vm_init
        .globals
        .borrow_mut(mc_in)
        .set("hit", vm_init.wrap_table(mc_in, hit));

    #[cfg(feature = "audio")]
    let sing_bpm = singer.clone();
    lua!(
        "bpm",
        move |_, _, tempo: f32| {
            #[cfg(feature = "audio")]
            {
                let _ = sing_bpm.send(SoundCommand::SetBpm(tempo));
            }
            Ok(())
        },
        "Set the transport tempo, shared by every channel",
        "
---@param tempo number beats per minute
function bpm(tempo) end"
    );

    #[cfg(feature = "audio")]
    let sing_cue = singer.clone();
    lua!(
        "cue",
        move |_,
              _,
              (freq, length, channel, instrument, at, grid): (
            f32,
            Option<f32>,
            Option<usize>,
            Option<usize>,
            Option<f32>,
            Option<f32>
        )| {
            #[cfg(feature = "audio")]
            {
                let len = length.unwrap_or(1.);
                let note = Note::new(instrument.unwrap_or(0), freq, len, 1.);
                // Beats go to the audio thread unconverted: it owns the tempo and the
                // frame counter, so a bpm change can't race a scheduled note.
                let _ = sing_cue.send(SoundCommand::PlayAt(
                    note,
                    channel,
                    at.unwrap_or(0.),
                    grid,
                ));
            }
            Ok(())
        },
        "Play a note on the transport: at a beat, or quantized to a grid",
        "
---@param freq number
---@param length number?
---@param channel integer?
---@param instrument integer?
---@param at number? absolute transport beat (default: as soon as possible)
---@param grid number? snap up to the next multiple of this many beats
function cue(freq, length, channel, instrument, at, grid) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "note",
        move |_,
              _,
              (freq, length, channel, instrument): (
            f32,
            Option<f32>,
            Option<usize>,
            Option<usize>
        )| {
            #[cfg(feature = "audio")]
            {
                let len = length.unwrap_or(1.);
                let note = Note::new(instrument.unwrap_or(0), freq, len, 1.);
                // `channel` None auto-allocates a free voice, so repeated note()
                // calls in a frame stack into a chord.
                let _ = sing.send(SoundCommand::PlayNote(note, channel));
            }
            Ok(())
        },
        "Play a note; optional channel + instrument. Overlapping notes voice separately.",
        "
---@param freq number
---@param length number?
---@param channel integer?
---@param instrument integer?
function note(freq, length, channel, instrument) end"
    );
    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "song",
        move |_, _, notes: Vec<Value>| {
            #[cfg(feature = "audio")]
            {
                let converted = notes
                    .iter()
                    .filter_map(|v| match v {
                        Value::Table(t) => {
                            let tb = t.borrow();
                            // {freq, len?, instrument?} — 1-indexed, matching the
                            // `cam` native's table reads. Empty table ([1] absent)
                            // is skipped.
                            match tb.getn(1) {
                                Some(f) => {
                                    let freq: f32 = f.into();
                                    let len: f32 = tb.getn(2).map(|v| v.into()).unwrap_or(1.);
                                    let instr: f32 = tb.getn(3).map(|v| v.into()).unwrap_or(0.);
                                    Some(Note::new(instr as usize, freq, len, 1.))
                                }
                                None => None,
                            }
                        }
                        Value::Number(n) => Some(Note::new(0, *n as f32, 1., 1.)),
                        Value::Integer(n) => Some(Note::new(0, *n as f32, 1., 1.)),
                        _ => None,
                    })
                    .collect::<Vec<Note>>();

                // A song is one sequential voice — auto-allocate a single channel.
                lua_err!(sing.send(SoundCommand::Chain(converted, None)));
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
        "chord",
        move |_, _, (freqs, length, instrument): (Vec<f32>, Option<f32>, Option<usize>)| {
            #[cfg(feature = "audio")]
            {
                let len = length.unwrap_or(1.);
                let instr = instrument.unwrap_or(0);
                // Each freq auto-allocates its own free voice → they sound together.
                for f in freqs {
                    let _ = sing.send(SoundCommand::PlayNote(Note::new(instr, f, len, 1.), None));
                }
            }
            Ok(())
        },
        "Play several frequencies at once as a chord",
        "
---@param freqs number[]
---@param length number?
---@param instrument integer?
function chord(freqs, length, instrument) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "mute",
        move |_, _, channel: Option<usize>| {
            #[cfg(feature = "audio")]
            // No channel = silence everything.
            let _ = sing.send(SoundCommand::Stop(channel));

            Ok(())
        },
        "Stop sounds on a channel, or all channels if omitted",
        "
---@param channel integer?
function mute(channel) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "sing",
        move |_, _, (lyrics, melody, length, channel): (String, Value, Option<f32>, Option<usize>)| {
            #[cfg(feature = "audio")]
            {
                // Formant-synth voice. `lyrics` is one syllable ("sa") or a
                // space-separated phrase ("la la laa"); `melody` is a single
                // pitch (all syllables) or a table of pitches / {freq,len} pairs
                // (one per syllable, index-clamped). Multiple syllables sequence
                // on one channel (like `song`); a single one plays immediately.
                let default_len = length.unwrap_or(0.5);
                // (freq, len) per melody step.
                let steps: Vec<(f32, f32)> = match &melody {
                    Value::Number(n) => vec![(*n as f32, default_len)],
                    Value::Integer(n) => vec![(*n as f32, default_len)],
                    Value::Table(t) => {
                        let tb = t.borrow();
                        let mut out = Vec::new();
                        let mut i = 1;
                        while let Some(v) = tb.getn(i) {
                            match v {
                                // {freq, len?}
                                Value::Table(pair) => {
                                    let p = pair.borrow();
                                    let f: f32 = p.getn(1).map(|x| x.into()).unwrap_or(440.0);
                                    let l: f32 =
                                        p.getn(2).map(|x| x.into()).unwrap_or(default_len);
                                    out.push((f, l));
                                }
                                // bare freq
                                other => out.push((other.into(), default_len)),
                            }
                            i += 1;
                        }
                        out
                    }
                    _ => vec![],
                };
                let syllables: Vec<&str> = lyrics.split_whitespace().collect();
                if !syllables.is_empty() && !steps.is_empty() {
                    let notes: Vec<Note> = syllables
                        .iter()
                        .enumerate()
                        .map(|(i, syl)| {
                            let (freq, len) = steps[i.min(steps.len() - 1)];
                            let syllable = crate::vocaloid::parse_syllable(syl);
                            Note::sung(freq, len, 1.0, &syllable)
                        })
                        .collect();
                    if notes.len() == 1 {
                        let _ = sing.send(SoundCommand::PlayNote(notes[0], channel));
                    } else {
                        // A phrase is one sequential voice on a single channel.
                        let _ = sing.send(SoundCommand::Chain(notes, channel));
                    }
                }
            }
            Ok(())
        },
        "Sing a syllable or space-separated phrase over a melody (a pitch, or a table of pitches / {freq,len} pairs)",
        "
---@param lyrics string a syllable ('sa') or phrase ('la la laa'); vowels a/e/i/o/u + optional consonant
---@param melody number|number[]|number[][] one pitch, or a pitch (or {freq,len}) per syllable
---@param length number? default per-syllable length
---@param channel integer?
function sing(lyrics, melody, length, channel) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "fade",
        move |_, _, (channel, secs, target): (usize, f32, Option<f32>)| {
            #[cfg(feature = "audio")]
            {
                // Channel-level effect: ramp a channel's output gain to `target`
                // over `secs` (Crossfade). Fade out (default 0), fade in (1), or
                // crossfade two channels with a pair of opposite fades.
                let _ =
                    sing.send(SoundCommand::FadeChannel(channel, secs, target.unwrap_or(0.0)));
            }
            Ok(())
        },
        "Fade a channel's volume to target (0..1, default 0) over secs — fade in/out or crossfade channels",
        "
---@param channel integer
---@param secs number fade duration in seconds
---@param target number? target gain 0..1 (default 0 = fade out)
function fade(channel, secs, target) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "echo",
        move |_, _, (channel, secs, feedback, mix): (usize, f32, Option<f32>, Option<f32>)| {
            #[cfg(feature = "audio")]
            {
                // Channel-level effect: a feedback delay. `secs` is the delay time
                // between echoes, `feedback` how much each echo carries into the
                // next (decay), `mix` the wet level. `secs <= 0` turns it off.
                let _ = sing.send(SoundCommand::EchoChannel(
                    channel,
                    secs,
                    feedback.unwrap_or(0.4),
                    mix.unwrap_or(0.5),
                ));
            }
            Ok(())
        },
        "Add a feedback-delay echo to a channel: (channel, secs, feedback 0..1, wet mix). secs<=0 disables it",
        "
---@param channel integer
---@param secs number delay time between echoes (<=0 disables)
---@param feedback number? echo decay per repeat 0..1 (default 0.4)
---@param mix number? wet level, echo loudness (default 0.5)
function echo(channel, secs, feedback, mix) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "filt",
        move |_,
              _,
              (channel, kind, cutoff, q, secs): (
            usize,
            Option<String>,
            Option<f32>,
            Option<f32>,
            Option<f32>
        )| {
            #[cfg(feature = "audio")]
            {
                // Channel-level resonant filter. `kind` picks the shape; `cutoff`
                // the corner/center Hz; `q` the resonance (0.707 flat, higher =
                // peak); `secs` optionally sweeps the cutoff there over time.
                // An unknown/absent/"off" kind disables the filter.
                let kind = match kind.as_deref().map(|s| s.to_ascii_lowercase()) {
                    Some(ref k) if k == "low" || k == "lp" || k == "lowpass" => Some(FilterKind::Low),
                    Some(ref k) if k == "high" || k == "hp" || k == "highpass" => {
                        Some(FilterKind::High)
                    }
                    Some(ref k) if k == "band" || k == "bp" || k == "bandpass" => {
                        Some(FilterKind::Band)
                    }
                    Some(ref k) if k == "notch" || k == "reject" => Some(FilterKind::Notch),
                    _ => None, // "off"/nil/unknown → disable
                };
                let _ = sing.send(SoundCommand::FilterChannel(
                    channel,
                    kind,
                    cutoff.unwrap_or(1000.0),
                    q.unwrap_or(0.707),
                    secs.unwrap_or(0.0),
                ));
            }
            Ok(())
        },
        "Resonant filter on a channel: (channel, kind 'low'|'high'|'band'|'notch', cutoff Hz, q, sweep secs). kind off/nil disables",
        "
---@param channel integer
---@param kind string? 'low' | 'high' | 'band' | 'notch' (off/nil disables)
---@param cutoff number? corner/center frequency Hz (default 1000)
---@param q number? resonance: 0.707 flat, higher peaks at cutoff (default 0.707)
---@param secs number? sweep the cutoff over this many seconds (default 0 = instant)
function filt(channel, kind, cutoff, q, secs) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "verb",
        move |_, _, (channel, room, damp, wet): (usize, f32, Option<f32>, Option<f32>)| {
            #[cfg(feature = "audio")]
            {
                // Channel-level reverb (a compact Freeverb). `room` is the tail
                // length/decay 0..1, `damp` rolls off the tail's highs 0..1, `wet`
                // the reverb level. `room <= 0` turns it off.
                let _ = sing.send(SoundCommand::ReverbChannel(
                    channel,
                    room,
                    damp.unwrap_or(0.5),
                    wet.unwrap_or(0.3),
                ));
            }
            Ok(())
        },
        "Add reverb to a channel: (channel, room/decay 0..1, damp 0..1, wet mix). room<=0 disables it",
        "
---@param channel integer
---@param room number tail length/decay 0..1 (<=0 disables)
---@param damp number? high-frequency damping of the tail 0..1 (default 0.5)
---@param wet number? reverb level mixed over the dry signal (default 0.3)
function verb(channel, room, damp, wet) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "mic",
        move |_, _, (id, secs): (Option<usize>, Option<f32>)| {
            // `mic(id, secs?)` records a snippet from the default input device
            // into sample slot `id`; `mic()` reports whether one is in flight.
            // Recording is always explicit — the engine never opens the mic on
            // its own, and the stream is released as soon as the capture lands.
            let (tx, rx) = std::sync::mpsc::sync_channel::<bool>(0);
            // `None` id is a pure query (is a capture running?) and never opens
            // the microphone; `Some(id)` starts one.
            lua_err!(pitcher.send((
                bundle_id,
                MainCommmand::MicRecord(id, secs.unwrap_or(1.0), tx)
            )));
            Ok(Value::Bool(rx.recv().unwrap_or(false)))
        },
        "Record a microphone snippet into a sample slot: mic(id, secs?). mic() = is a capture running?",
        "
---@param id integer? sample/instrument slot to record into (omit to query)
---@param secs number? capture length in seconds (default 1, max 10)
---@return boolean started (or, with no args, whether a capture is running)
function mic(id, secs) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "midi",
        move |lu, mc, (action, name): (Option<String>, Option<String>)| {
            #[cfg(all(feature = "midi", not(target_arch = "wasm32")))]
            {
                // One native, selected by the (optional) first argument:
                //   midi()          -> drain pending events (the per-frame call)
                //   midi('ports')   -> list available input port names
                //   midi('port')    -> the connected port's name, or nil
                //   midi('open', n?)-> connect (n matches a port name substring)
                //   midi('close')   -> disconnect
                match action.as_deref().map(|s| s.to_ascii_lowercase()) {
                    None => {
                        // Events as an array of {status, channel, data1, data2}.
                        let mut out = lu.raw_table();
                        for (i, (status, ch, d1, d2)) in
                            crate::midi::drain().into_iter().enumerate()
                        {
                            let mut ev = lu.raw_table();
                            ev.set(1, Value::Integer(status as i64));
                            ev.set(2, Value::Integer(ch as i64));
                            ev.set(3, Value::Integer(d1 as i64));
                            ev.set(4, Value::Integer(d2 as i64));
                            out.set(i as i64 + 1, lu.wrap_table(mc, ev));
                        }
                        return Ok(lu.wrap_table(mc, out));
                    }
                    Some(ref a) if a == "ports" => {
                        let mut out = lu.raw_table();
                        for (i, p) in crate::midi::ports().into_iter().enumerate() {
                            out.set(i as i64 + 1, Value::String(p));
                        }
                        return Ok(lu.wrap_table(mc, out));
                    }
                    Some(ref a) if a == "port" => {
                        return Ok(match crate::midi::connected() {
                            Some(p) => Value::String(p),
                            None => Value::Nil,
                        });
                    }
                    Some(ref a) if a == "open" => {
                        return Ok(
                            match crate::midi::open(sing.clone(), name.as_deref()) {
                                Ok(p) => Value::String(p),
                                Err(_) => Value::Nil,
                            },
                        );
                    }
                    Some(ref a) if a == "close" => {
                        crate::midi::close();
                        return Ok(Value::Nil);
                    }
                    _ => return Ok(Value::Nil),
                }
            }
            #[cfg(not(all(feature = "midi", not(target_arch = "wasm32"))))]
            Ok(Value::Nil)
        },
        "MIDI in: midi() drains events, midi('ports'|'port'), midi('open', name?), midi('close')",
        "
---@param action string? nil = drain events; 'ports' | 'port' | 'open' | 'close'
---@param name string? port name substring for 'open'
---@return table|string|nil
function midi(action, name) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "crsh",
        move |_, _, (channel, bits, rate): (usize, f32, Option<f32>)| {
            #[cfg(feature = "audio")]
            {
                // Channel-level bitcrusher: quantize to `bits` of depth and
                // (optionally) latch at a lower `rate` in Hz. `bits <= 0` disables.
                let _ = sing.send(SoundCommand::CrushChannel(
                    channel,
                    bits,
                    rate.unwrap_or(0.0),
                ));
            }
            Ok(())
        },
        "Bitcrush a channel: (channel, bits 1..16, rate Hz). bits<=0 disables it",
        "
---@param channel integer
---@param bits number target bit depth 1..16 (<=0 disables, 16 = no quantizing)
---@param rate number? target sample rate in Hz for decimation (default 0 = none)
function crsh(channel, bits, rate) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "grit",
        move |_, _, (channel, amount, mode): (usize, f32, Option<String>)| {
            #[cfg(feature = "audio")]
            {
                // Channel-level drive/distortion: push the signal into a
                // waveshaping curve. `amount <= 0` disables it.
                let shape = match mode.as_deref().map(|s| s.to_ascii_lowercase()) {
                    Some(ref m) if m == "hard" || m == "clip" => DriveShape::Hard,
                    Some(ref m) if m == "fold" || m == "foldback" => DriveShape::Fold,
                    _ => DriveShape::Soft, // default / 'soft'
                };
                let _ = sing.send(SoundCommand::DriveChannel(channel, amount, shape));
            }
            Ok(())
        },
        "Drive/distort a channel: (channel, amount 0..1, curve 'soft'|'hard'|'fold'). amount<=0 disables it",
        "
---@param channel integer
---@param amount number drive amount 0..1 (<=0 disables)
---@param mode string? curve: 'soft' (default, tanh) | 'hard' (clip) | 'fold' (foldback)
function grit(channel, amount, mode) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "vox",
        move |_, _, (a, b): (Value, Option<Value>)| {
            #[cfg(feature = "audio")]
            {
                // Per-channel singing-voice character for that channel's later
                // `sing` notes: breath (aspiration noise) + vibrato (pitch wobble).
                // Channel-scoped like `fade`, so different channels = different
                // singers. Accepts `vox(channel, cfg)` or `vox(cfg)` (channel 0):
                // if the first arg is a number it's the channel and `b` is the cfg
                // table; if it's a table it's the cfg for channel 0.
                let (ch, cfg) = match &a {
                    Value::Number(n) => (*n as usize, b.unwrap_or(Value::Nil)),
                    Value::Integer(n) => (*n as usize, b.unwrap_or(Value::Nil)),
                    _ => (0usize, a.clone()),
                };
                let mut breath = 0.0f32;
                let mut vib = 0.0f32;
                let mut hz = 5.5f32;
                if let Value::Table(t) = &cfg {
                    let tb = t.borrow();
                    if let Some(v) = tb.get("breath") {
                        breath = v.into();
                    }
                    if let Some(v) = tb.get("vib") {
                        vib = v.into();
                    }
                    if let Some(v) = tb.get("hz") {
                        hz = v.into();
                    }
                }
                let _ = sing.send(SoundCommand::VoiceConfig(ch, breath, vib, hz));
            }
            Ok(())
        },
        "Set a channel's singing voice character for its later sing() notes: (channel, { breath, vib, hz })",
        "
---@param channel integer? channel to configure (default 0)
---@param cfg { breath?: number, vib?: number, hz?: number } breath 0..1, vibrato depth (~0.03), rate Hz (~5.5)
function vox(channel, cfg) end"
    );

    #[cfg(feature = "audio")]
    let sing = singer.clone();
    lua!(
        "smpl",
        move |_, _, (id, data, cfg): (usize, Value, Option<Value>)| {
            #[cfg(feature = "audio")]
            {
                // cfg (optional): { base=, atk=, dec=, sus=, rel= }, or a bare
                // number for legacy base pitch. `base` is the natural-playback Hz.
                let (env, base) = parse_sound_cfg(cfg.as_ref(), "base");
                match data {
                    // A name binds a sound loaded from sounds/<name>.ogg into slot
                    // `id`. The lookup happens once, here — the mixer stays purely
                    // index-keyed.
                    Value::String(name) => {
                        lua_err!(sing.send(SoundCommand::BindSample(
                            id,
                            name.to_string(),
                            base,
                            env
                        )));
                    }
                    // A table is raw PCM. Read it by index (getn loop) rather than
                    // Vec<f32> FromLua so we don't hit silt's hash-order Table->Vec
                    // bug — sample order is critical, a scrambled buffer is noise.
                    Value::Table(t) => {
                        let tb = t.borrow();
                        let mut pcm: Vec<f32> = Vec::new();
                        let mut i = 1;
                        while let Some(v) = tb.getn(i) {
                            pcm.push(v.into());
                            i += 1;
                        }
                        lua_err!(sing.send(SoundCommand::MakeSample(
                            id,
                            pcm,
                            base.unwrap_or(440.0),
                            env
                        )));
                    }
                    _ => {}
                }
            }
            Ok(())
        },
        "Define instrument `id` from a loaded sound name, or raw PCM samples (-1..1); cfg = { base, atk, dec, sus, rel }",
        "
---@param id integer
---@param data string|number[] a loaded sound name (sounds/<name>.ogg), or raw PCM samples in -1..1
---@param cfg? { base?: number, atk?: number, dec?: number, sus?: number, rel?: number } base pitch (default 440) + ADSR envelope
function smpl(id, data, cfg) end"
    );

    lua!(
        "instr",
        move |_, _, (id, spec, cfg): (usize, Value, Option<Value>)| {
            #[cfg(feature = "audio")]
            {
                // cfg (optional): { wid=, atk=, dec=, sus=, rel= }, or a bare
                // number for legacy pulse width. `wid` is the pulse duty.
                let (env, wid) = parse_sound_cfg(cfg.as_ref(), "wid");
                // spec is either a waveform name ('square', 'saw', 'tri', 'pulse',
                // 'noise', 'sine') or a table of harmonic amplitudes (additive).
                let inst = match spec {
                    Value::String(s) => {
                        let wave = match s.to_lowercase().as_str() {
                            "sine" | "sin" => WaveType::Sine,
                            "saw" => WaveType::Saw,
                            "tri" | "triangle" => WaveType::Triangle,
                            "pulse" => WaveType::Pulse(wid.unwrap_or(0.5)),
                            "noise" => WaveType::Noise,
                            _ => WaveType::Square, // 'square'/'sqr' and fallback
                        };
                        Instrument::oscillator(id, wave)
                    }
                    Value::Table(t) => {
                        let tb = t.borrow();
                        let mut amps = Vec::new();
                        let mut i = 1;
                        while let Some(v) = tb.getn(i) {
                            amps.push(v.into());
                            i += 1;
                        }
                        Instrument::additive(id, amps)
                    }
                    _ => Instrument::oscillator(id, WaveType::Square),
                }
                .with_env(env);
                lua_err!(singer.send(SoundCommand::MakeInstrument(inst)));
            }
            Ok(())
        },
        "Define instrument `id`: a waveform name (square/saw/tri/pulse/noise/sine) or a harmonic-amplitude table; cfg = { wid, atk, dec, sus, rel }",
        "
---@param id integer
---@param spec string|number[] waveform name, or harmonic amplitudes (additive)
---@param cfg? { wid?: number, atk?: number, dec?: number, sus?: number, rel?: number } pulse duty + ADSR envelope
function instr(id, spec, cfg) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "tex",
        move |_, _, (name, im): (String, Value)| {
            if let Value::UserData(ud) = im {
                ud.borrow().downcast_ref::<LuaImg, _, _>(|limg| {
                    // Fire and forget, on every target (wasm always did: its VM
                    // can't block on the main thread). Commands arrive in order, so
                    // a mod() or make() after this already sees the texture; the
                    // reply slot is buffered so the main thread never waits either.
                    let (tx, _rx) = std::sync::mpsc::sync_channel::<()>(1);
                    lua_err!(pitcher.send((
                        bundle_id,
                        MainCommmand::SetImg(name, limg.image.clone(), tx),
                    )));
                    Ok(())
                })?;
            };

            Ok(())
        },
        "Sets image data as a texture",
        "
---@param asset string
---@param im image
function tex(asset, im) end"
    );

    // vm.create_userdata(mc, data)
    let pitcher = main_pitcher.clone();
    lua!(
        "gimg",
        move |vm, mc, name: String| {
            let (tx, rx) = std::sync::mpsc::sync_channel::<(u32, u32, RgbaImage)>(0);
            let limg = match pitcher.send((bundle_id, MainCommmand::GetImg(name, tx))) {
                Ok(_o) => match rx.recv() {
                    Ok((w, h, im)) => {
                        let lua_img =
                            LuaImg::new(bundle_id, im, w, h, gui.borrow().letters.clone());
                        lua_img
                    }
                    _ => LuaImg::empty(),
                },
                _ => LuaImg::empty(),
            };
            Ok(vm.create_userdata(mc, limg))
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
        move |vm, mc, (w, h): (u32, u32)| {
            let im = GuiMorsel::new_image(w, h);
            let lua_img = LuaImg::new(bundle_id, im, w, h, gui.borrow().letters.clone());
            // let ud = lua_img_constructor(lua_img);

            Ok(vm.create_userdata(mc, lua_img))
        },
        "Create new image buffer userdata, does not set as asset",
        "
---@param w integer
---@param h integer
---@return image
function nimg(w, h) end"
    );

    let pitcher = main_pitcher.clone();
    let tiles = world_sender.clone();
    let tile_named = std::rc::Rc::new(std::cell::RefCell::new(std::collections::HashSet::<String>::new()));
    lua!(
            "mod",
            move |vm,mc, (asset, tval): (String, Value)| {
                // The reply slot is buffered and nobody waits on it: commands from this
                // thread reach the main thread in order, so a `make()` or `gmod()` after
                // this already sees the model. Waiting for the ack used to cost a whole
                // main-loop pass per call, which capped a tool rebuilding a mesh every
                // frame at a fraction of the frame rate.
                let (tx, _rx) = std::sync::mpsc::sync_channel::<u8>(1);
                let t=if let Value::Table(t)=tval{t.borrow()}else{return 

                                Err(static_err("expecting table for param 2"));};
                // Name the model to this app's tiles now, on the channel tile()
                // uses: the model itself reaches the main thread later, and a
                // tile() placed in between found no model by that name and drew
                // the fallback texture instead. The main thread's own mapping,
                // when the model lands, reuses this index. Once per name: a
                // tool rebuilding a mesh every frame asks only the first time.
                let low = asset.to_lowercase();
                if tile_named.borrow_mut().insert(low.clone()) {
                    let (mtx, mrx) = sync_channel::<TileResponse>(1);
                    if tiles.send((TileCommand::MapModel(low), mtx)).is_ok() {
                        let _ = mrx.recv();
                    }
                }

                if let Some(built) = mesh_from_table(&t, vm, mc) {
                    let (mut mesh, names) = built.map_err(static_err)?;
                    if mesh.pos.len() < 3 {
                        return Err(static_err("mod: nothing to build (no faces or segments)"));
                    }
                    let mut textures = ordered_list::<String>(&t, "t", vm, mc)
                        .unwrap_or_else(|| vec![crate::texture::DEFAULT_TEX.to_string()]);
                    if !names.is_empty() {
                        // A texture per face: the model code cycles its texture
                        // list one per three vertices, so hand it unshared
                        // triangles and a name for each.
                        let (soup, tri_tex) = mesh.soup();
                        textures = tri_tex.iter().map(|&i| names[i as usize].clone()).collect();
                        mesh = soup;
                    }
                    lua_err!(pitcher.send((
                        bundle_id,
                        MainCommmand::Model(Box::new(ModelPacket {
                            asset,
                            textures,
                            vecs: mesh.pos,
                            norms: mesh.norm,
                            inds: mesh.ind,
                            uvs: mesh.uv,
                            style: TextureStyle::Tri,
                            sender: tx,
                        })),
                    )));
                    return Ok("Building model in polygon mode");
                }

                match ordered_list::<[f32; 3]>(&t,"q",vm,mc) {
                    Some(quads) => {

                        let (v,n, uv, i) =
                        match ordered_list::<[f32;2]>(&t,"u",vm,mc) { //Vec<[f32;2]>
                            Some(uvi)=>{
                                let inds= ordered_list::<u32>(&t,"i",vm,mc).unwrap_or_else(||{ 
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
                                let norms=ordered_list::<[f32;3]>(&t,"n",vm,mc).unwrap_or_else(||{
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
                        match ordered_list::<String>(&t,"t",vm,mc) {
                            Some(textures) => {
                                // let tt=Vec::<String>::from_lua(tex, vm, mc);
                                // let tt: Vec<String>= textures.clone().
                                lua_err!(pitcher.send((
                                    bundle_id,
                                    MainCommmand::Model(
                                        Box::new(ModelPacket{
                                            asset,
                                            textures,
                                            vecs:v,
                                            norms:n,
                                            inds:i,
                                            uvs:uv,
                                            style:TextureStyle::Quad,
                                            sender:tx,
                                        })
                                    )),
                                ));
                            }
                            _ => {
                                // No texture given: build with the engine's fallback
                                // rather than refusing. A model is a mesh; its texture
                                // can be set afterwards through the entity. This branch
                                // used to construct an Err as a bare expression and drop
                                // it, then fall through to Ok — so the model was never
                                // built and Lua was told it had been, which looks
                                // exactly like a camera pointing the wrong way.
                                lua_err!(pitcher.send((
                                    bundle_id,
                                    MainCommmand::Model(Box::new(ModelPacket {
                                        asset,
                                        textures: vec![crate::texture::DEFAULT_TEX.to_string()],
                                        vecs: v,
                                        norms: n,
                                        inds: i,
                                        uvs: uv,
                                        style: TextureStyle::Quad,
                                        sender: tx,
                                    }))
                                )));
                            }
                        }
                        Ok("Building model in quad mode")
                    }
                    _ => {
                        // println!("got no quads");
                        let vin = ordered_list::<[f32; 3]>(&t,"v",vm,mc);
                        match vin {
                            Some(vecs) => {
                                if vecs.len() > 2 {
                                    let inds =  ordered_list::<u32>(&t,"i",vm,mc).unwrap_or_default();
                                    let uvs =  ordered_list::<[f32; 2]>(&t,"u",vm,mc).unwrap_or_default();
                                    let norms =  ordered_list::<[f32; 3]>(&t,"n",vm,mc).unwrap_or_default();
                                    match ordered_list::<String>(&t,"t",vm,mc) {
                                        Some(textures) => {
                                            lua_err!( pitcher.send((
                                                bundle_id,
                                                MainCommmand::Model(
                                                    Box::new(ModelPacket{
                                                    asset,
                                                    textures,
                                                    vecs,
                                                    norms,
                                                    inds,
                                                    uvs,
                                                    style:TextureStyle::Tri,
                                                    sender:tx})
                                                ),
                                            )));
                                            return Ok("Building model in vert mode")
                                        }
                                        _ => {
                                            // Same fallback as the quad form: a mesh
                                            // without a texture is still a mesh.
                                            lua_err!(pitcher.send((
                                                bundle_id,
                                                MainCommmand::Model(Box::new(ModelPacket{
                                                    asset,
                                                    textures: vec![crate::texture::DEFAULT_TEX.to_string()],
                                                    vecs,
                                                    norms,
                                                    inds,
                                                    uvs,
                                                    style: TextureStyle::Tri,
                                                    sender: tx})
                                                ),
                                            )));
                                            return Ok("Building model in vert mode")
                                        }
                                    }
                                }
                                Err(static_err("This type of model requires a vertex list at index \"v\" < v={{0,0,0},{1,0,0},{1,1,0},{0,1,0}} >"))
                            }
                            _ => {
                                match ordered_list::<String>(&t,"t",vm,mc) {
                                    Some(texture) => {
                                        if texture.len() > 0 {
                                            let t = texture[0].clone();
                                            lua_err!(pitcher.send((
                                                bundle_id,
                                                MainCommmand::Make(
                                                    vec![
                                                        asset,
                                                        t.clone(),
                                                        texture.get(1).unwrap_or(&t).to_string(),
                                                        texture.get(2).unwrap_or(&t).to_string(),
                                                        texture.get(3).unwrap_or(&t).to_string(),
                                                        texture.get(4).unwrap_or(&t).to_string(),
                                                        texture.get(5).unwrap_or(&t).to_string(),
                                                    ],
                                                    tx,
                                                ),
                                            )));
                                        }
                                        Ok("Building model in texture mode")
                                    }
                                    _ => {
                                        // Err::<(),&str>
                                        Err(static_err("This type of model requires a texture at index \"t\" < t='name_of_image_without_extension' >"))
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
        move |_, _, (model, bundle): (Option<String>, Option<u8>)| {
            let (tx, rx) = sync_channel::<Vec<String>>(0);
            lua_err!(pitcher.send((
                bundle_id,
                MainCommmand::ListModel(model.unwrap_or("".to_string()), bundle, tx),
            )));
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

    // Lay a mesh flat onto an image: the paint overlay's NEW uses the same
    // layout. Vertices on chart borders are split, so the result has its own
    // vertex list; `from` says which input vertex each came from.
    lua!(
        "unwrap",
        move |vm, mc, (mesh, opts): (Value, Value)| {
            let Value::Table(m) = &mesh else {
                return Err(static_err("unwrap: first argument must be a mesh table {v, i}"));
            };
            let m = m.borrow();
            let pos = ordered_list::<[f32; 3]>(&m, "v", vm, mc).unwrap_or_default();
            let norm = ordered_list::<[f32; 3]>(&m, "n", vm, mc).unwrap_or_default();
            let mut ind = ordered_list::<u32>(&m, "i", vm, mc).unwrap_or_default();
            drop(m);
            if ind.is_empty() {
                ind = (0..pos.len() as u32).collect();
            }
            let (mut w, mut h, mut pad, mut mode) = (64u32, 64u32, 1u32, crate::unwrap::Mode::Box);
            if let Value::Table(o) = &opts {
                let o = o.borrow();
                let num = |k: &str| value_f32(&o.get_value(&k.into())).map(|f| f.max(0.) as u32);
                w = num("w").unwrap_or(w).max(1);
                h = num("h").unwrap_or(h).max(1);
                pad = num("pad").unwrap_or(pad);
                if let Value::String(s) = o.get_value(&"mode".into()) {
                    mode = crate::unwrap::Mode::parse(&s.to_string())
                        .ok_or_else(|| static_err("unwrap: mode is \"box\" or \"flat\""))?;
                }
            }
            let un = crate::unwrap::unwrap(&pos, &ind, mode, w, h, pad);
            let v: Vec<Value> = un
                .pos
                .iter()
                .map(|p| vm.table_from_array(mc, [p[0] as f64, p[1] as f64, p[2] as f64]))
                .collect();
            let u: Vec<Value> = un
                .uv
                .iter()
                .map(|p| vm.table_from_array(mc, [p[0] as f64, p[1] as f64]))
                .collect();
            let i: Vec<Value> = un.ind.iter().map(|i| Value::Integer(*i as i64)).collect();
            let from: Vec<Value> = un.from.iter().map(|i| Value::Integer(*i as i64)).collect();
            let mut out = vm.raw_table();
            out.set("v", vm.table_from_array(mc, v));
            out.set("u", vm.table_from_array(mc, u));
            out.set("i", vm.table_from_array(mc, i));
            out.set("from", vm.table_from_array(mc, from));
            if norm.len() == pos.len() {
                let n: Vec<Value> = un
                    .from
                    .iter()
                    .map(|&f| {
                        let p = norm[f as usize];
                        vm.table_from_array(mc, [p[0] as f64, p[1] as f64, p[2] as f64])
                    })
                    .collect();
                out.set("n", vm.table_from_array(mc, n));
            }
            out.set("charts", un.charts as i64);
            Ok(vm.wrap_table(mc, out))
        },
        "Lay a mesh's triangles flat onto a w x h image: new v/n/u/i to pass to mod() <mesh:{v,i,n?}, opts?:{w,h,pad,mode}>",
        "
---@param mesh {v:number[3][], i?:integer[], n?:number[3][]}
---@param opts? {w?:integer, h?:integer, pad?:integer, mode?:string}
---@return {v:number[3][], n:number[3][]?, u:number[2][], i:integer[], from:integer[], charts:integer}
function unwrap(mesh, opts) end"
    );

    // glTF out of, and back into, the same arrays mod() takes. Writing is how a
    // modelling tool saves its work as an asset the engine (or anything else) can
    // load; reading hands a file's triangles back as tables so a tool can edit an
    // existing model. The heavy lifting is in gltf_io.rs; this only moves tables.
    let pitcher = main_pitcher.clone();
    lua!(
        "glb",
        move |vm, mc, (path, data): (String, Value)| {
            let opts = match &data {
                Value::Table(t) => Some(t.borrow()),
                Value::Nil => None,
                _ => return Err(static_err("glb: second argument must be a table")),
            };
            let y_up = opts
                .as_ref()
                .and_then(|t| ordered_list::<String>(t, "up", vm, mc))
                .map(|u| u.first().map(|s| s.eq_ignore_ascii_case("y")).unwrap_or(false))
                .unwrap_or(false);
            let writing = opts
                .as_ref()
                .map(|t| !matches!(t.get_value(&"v".into()), Value::Nil))
                .unwrap_or(false);

            let shaped = opts.as_ref().and_then(|t| mesh_from_table(t, vm, mc));
            if let Some(built) = shaped {
                let t = opts.unwrap();
                let (mut mesh, names) = built.map_err(static_err)?;
                // ft names every texture the faces use (baked into one image on
                // write); otherwise t is the one texture
                let texture = if names.is_empty() {
                    ordered_list::<String>(&t, "t", vm, mc)
                        .and_then(|v| v.into_iter().next())
                        .into_iter()
                        .collect()
                } else {
                    names
                };
                drop(t);
                if let Err(e) = mesh.complete() {
                    return Err(static_err(e));
                }
                if y_up {
                    mesh.to_y_up();
                }
                let (tx, rx) = sync_channel::<bool>(0);
                lua_err!(pitcher.send((
                    bundle_id,
                    MainCommmand::GlbWrite(Box::new((path, mesh, texture)), tx)
                )));
                return Ok(Value::Bool(rx.recv().unwrap_or(false)));
            }

            if writing {
                let t = opts.unwrap();
                let mut mesh = crate::gltf_io::MeshData {
                    pos: ordered_list::<[f32; 3]>(&t, "v", vm, mc).unwrap_or_default(),
                    norm: ordered_list::<[f32; 3]>(&t, "n", vm, mc).unwrap_or_default(),
                    uv: ordered_list::<[f32; 2]>(&t, "u", vm, mc).unwrap_or_default(),
                    ind: ordered_list::<u32>(&t, "i", vm, mc).unwrap_or_default(),
                    tex: vec![],
                };
                let texture: Vec<String> = ordered_list::<String>(&t, "t", vm, mc)
                    .and_then(|v| v.into_iter().next())
                    .into_iter()
                    .collect();
                drop(t);
                if let Err(e) = mesh.complete() {
                    return Err(static_err(e));
                }
                if y_up {
                    mesh.to_y_up();
                }
                let (tx, rx) = sync_channel::<bool>(0);
                lua_err!(pitcher.send((
                    bundle_id,
                    MainCommmand::GlbWrite(Box::new((path, mesh, texture)), tx)
                )));
                return Ok(Value::Bool(rx.recv().unwrap_or(false)));
            }

            let (tx, rx) = sync_channel::<Option<crate::gltf_io::MeshData>>(0);
            lua_err!(pitcher.send((bundle_id, MainCommmand::GlbRead(path, tx))));
            let mut mesh = match rx.recv() {
                Ok(Some(m)) => m,
                _ => return Ok(Value::Nil),
            };
            if y_up {
                mesh.from_y_up();
            }
            let v3 = |a: &Vec<[f32; 3]>| -> Vec<Value> {
                a.iter()
                    .map(|p| vm.table_from_array(mc, [p[0] as f64, p[1] as f64, p[2] as f64]))
                    .collect()
            };
            let pos = v3(&mesh.pos);
            let norm = v3(&mesh.norm);
            let uv: Vec<Value> = mesh
                .uv
                .iter()
                .map(|p| vm.table_from_array(mc, [p[0] as f64, p[1] as f64]))
                .collect();
            let ind: Vec<Value> = mesh.ind.iter().map(|i| Value::Integer(*i as i64)).collect();
            let mut out = vm.raw_table();
            out.set("v", vm.table_from_array(mc, pos));
            out.set("n", vm.table_from_array(mc, norm));
            out.set("u", vm.table_from_array(mc, uv));
            out.set("i", vm.table_from_array(mc, ind));
            Ok(vm.wrap_table(mc, out))
        },
        "Write mesh tables to a .glb/.gltf in the app folder, or read one back <path:string, data?:{v,i,n,u,t,up}>",
        "
---@param path string
---@param data {v:number[3][], i?:integer[], n?:number[3][], u?:number[2][], t?:string, up?:string}?
---@return boolean|{v:number[3][], i:integer[], n:number[3][], u:number[2][]}|nil
function glb(path, data) end"
    );

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
        move |_, _, (a, b): (Option<f64>, Option<f64>)| {
            match a {
                Some(fa) => match b {
                    Some(fb) => Ok(fastrand::f64() * (fb - fa) + fa),
                    _ => Ok(fastrand::f64() * fa),
                },
                _ => Ok(fastrand::f64()),
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
        move |_, _, (a, b): (Option<i64>, Option<i64>)| {
            match a {
                Some(fa) => match b {
                    Some(fb) => Ok(fastrand::i64(fa..fb)),
                    _ => Ok(fastrand::i64(0..fa)),
                },
                _ => Ok(fastrand::i64(..)),
            }
        },
        "An imperfect random number generator for integers. May suffer from modulo bias.",
        "
---@param a integer?
---@param b integer?
---@return integer
function irnd(a, b) end"
    );

    lua!(
        "now",
        move |_, _, (): ()| { Ok(crate::skeleton::clock()) },
        "Seconds since the engine started, as of this frame. Steps are paced by the display, so use the difference between frames to move things at a steady speed",
        "
---@return number
function now() end"
    );

    lua!(
        "flr",
        move |_, _, f: f64| { Ok(f.floor() as i64) },
        "Floor value",
        "
---@param f number
---@return integer
function flr(f) end"
    );

    lua!(
        "ceil",
        move |_, _, f: f64| { Ok(f.ceil() as i64) },
        "Ceil value",
        "
---@param f number
---@return integer
function ceil(f) end"
    );

    lua!(
        "rou",
        move |_, _, f: f64| { Ok(f.round() as i64) },
        "Round value",
        "
---@param f number
---@return integer
function rou(f) end"
    );

    lua!(
        "abs",
        move |_, _, f: f64| { Ok(f.abs()) },
        "Absolute value",
        "
---@param f number
---@return number
function abs(f) end"
    );

    lua!(
        "cos",
        move |_, _, f: f64| { Ok(f.cos()) },
        "Cosine value",
        "
---@param f number  
---@return number
function cos(f) end"
    );
    lua!(
        "sin",
        move |_, _, f: f64| { Ok(f.sin()) },
        "Sine value",
        "
---@param f number
---@return number
function sin(f) end"
    );
    lua!(
        "sqrt",
        move |_, _, f: f64| { Ok(f.sqrt()) },
        "Squareroot value",
        "
---@param f number
---@return number
function sqrt(f) end"
    );
    lua!(
        "atan2",
        move |_, _, (y, x): (f64, f64)| { Ok(y.atan2(x)) },
        "Angle of the point (x, y) from the +x axis",
        "
---@param y number
---@param x number
---@return number
function atan2(y, x) end"
    );

    lua!(
        "pow",
        move |_, _, (f, e): (f64, f64)| { Ok(f.powf(e)) },
        "Squareroot value",
        "
---@param f number target
---@param e number raise by
---@return number
function pow(f,e) end"
    );
    lua!(
        "log",
        move |_, _, f: f64| { Ok(f.log10()) },
        "Base 10 logarithm of the value",
        "
---@param f number target
---@return number
function log(f) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "sub",
        move |_, _, str: String| {
            lua_err!(pitcher.send((bundle_id, MainCommmand::Subload(str, false))));
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
        move |_, _, str: String| {
            lua_err!(pitcher.send((bundle_id, MainCommmand::Subload(str, true))));
            Ok(())
        },
        "load an overlaying bundle",
        "   
---@param str string
function over(str) end"
    );

    lua!(
        "conn",
        move |vm, mc, (addr, udp, server): (String, Option<bool>, Option<bool>)| {
            #[cfg(any(feature = "ws", feature = "online_capable"))]
            {
                return match crate::lua_connection::LuaConnection::open(
                    &addr,
                    udp.unwrap_or(false),
                    server.unwrap_or(false),
                ) {
                    Ok(c) => Ok(vm.create_userdata(mc, c)),
                    Err(er) => Err(LuaError::Custom(format!("conn fail, {}", er))),
                };
            }
            // No transport compiled in: nil, as before, so `if socket then` holds.
            #[cfg(not(any(feature = "ws", feature = "online_capable")))]
            {
                let _ = (vm, mc, addr, udp, server);
                Ok(Value::Nil)
            }
        },
        "Open a connection: ws://, wss:// or /path for a websocket, else ip:port tcp",
        "
--- @param addr string
--- @param udp? boolean
--- @param server? boolean
--- @return connection
function conn(addr,udp,server) end"
    );

    let pitcher = main_pitcher.clone();
    lua!(
        "quit",
        move |_, _, u: Option<u8>| {
            lua_err!(pitcher.send((bundle_id, MainCommmand::Quit(u.unwrap_or(0)))));
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
        move |vm, mc, b: bool| {
            println!("we got inside help command");
            let mut t = vm.raw_table();
            t.set("help", "list all lua commands. In fact, the command used by this program to list this very command");
            for (k, (desc, examp)) in command_map_clone.iter() {
                if b {
                    t.set(
                        k.to_string(),
                        vm.table_from_array(mc, [desc.to_string(), examp.to_string()]),
                    );
                } else {
                    t.set(k.to_string(), desc.to_string());
                }
            }
            println!("we made a help table");
            Ok(vm.wrap_table(mc, t))
        },
        "List all commands",
        "
---@return table
function help() end"
    );

    let pitcher = main_pitcher.clone();
    lua_lib!(
        "get",
        move |_, _, file: String| {
            let (tx, rx) = sync_channel::<Option<String>>(0);
            lua_err!(pitcher.send((bundle_id, MainCommmand::Read(file, tx))));
            match rx.recv() {
                Ok(o) => match o {
                    Some(s) => Ok(Value::String(s)),
                    None => Ok(Value::Nil),
                },
                _ => Ok(Value::Nil),
            }
        },
        "load a file relative to the game directory",
        "
---@return table
function get() end",
        &mut io
    );

    let pitcher = main_pitcher.clone();
    lua_lib!(
        "set",
        move |_, _, (file, contents): (String, String)| {
            let (tx, rx) = sync_channel::<bool>(0);
            lua_err!(pitcher.send((bundle_id, MainCommmand::Write(file, contents, tx))));
            match rx.recv() {
                Ok(o) => Ok(Value::Bool(o)),
                Err(_) => Ok(Value::Bool(false)),
            }
        },
        "load a file relative to the game directory",
        "
---@return table
function get() end",
        &mut io
    );

    let pitcher = main_pitcher.clone();
    lua_lib!(
        "copy",
        move |_, _, content: String| {
            lua_err!(pitcher.send((bundle_id, MainCommmand::Copy(content))));
            Ok(())
        },
        "Copy text to the clipboard",
        "
---@return table
function help() end",
        &mut io
    );

    // lua!(
    //     "test420",
    //     move |lu, b: Table| {
    //         crate::packet::table_serial(b);
    //         Ok(())
    //     },
    //     "Test 4/20/2023",
    //     ""
    // );

    vm_init
        .globals
        .borrow_mut(mc_in)
        .set("io", vm_init.wrap_table(mc_in, io));

    // if let Err(e) = lua_globals.set(*ctx, "io", io) {
    //     return Err(context_err("Failed to set io lib"));
    // }

    // An overlay's reach into the app it edits — read a source file, write it back,
    // list what's there, and make the app pick the change up.
    //
    // This table is built only for a bundle the *engine* marked as an overlay, so in
    // game Lua it doesn't exist at all: there's no native to hide behind a flag and
    // no way for an app to ask for one, because an app cannot create an overlay (its
    // `over()` makes a child bundle, and only the console and `--overlay` mark the
    // real thing). The main thread checks the sender is an overlay a second time when
    // these arrive — see `MainCommmand::AppRead`. Editing someone's files deserves
    // both locks.
    if is_overlay {
        let mut app = vm_init.raw_table();

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "read",
            move |_, _, file: String| {
                let (tx, rx) = sync_channel::<Option<String>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppRead(file, tx))));
                match rx.recv() {
                    Ok(Some(s)) => Ok(Value::String(s)),
                    _ => Ok(Value::Nil),
                }
            },
            "overlay only: read a file out of the app being edited",
            "
---@param path string
---@return string|nil
function app.read(path) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "write",
            move |_, _, (file, contents): (String, String)| {
                let (tx, rx) = sync_channel::<bool>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppWrite(file, contents, tx))));
                match rx.recv() {
                    Ok(o) => Ok(Value::Bool(o)),
                    Err(_) => Ok(Value::Bool(false)),
                }
            },
            "overlay only: write a file into the app being edited",
            "
---@param path string
---@param contents string
---@return boolean
function app.write(path, contents) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "png",
            move |_, _, (texture, file): (String, String)| {
                let (tx, rx) = sync_channel::<bool>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppPng(texture, file, tx))));
                match rx.recv() {
                    Ok(o) => Ok(Value::Bool(o)),
                    Err(_) => Ok(Value::Bool(false)),
                }
            },
            "overlay only: write a texture out as a png file in the app being edited",
            "
---@param texture string
---@param path string
---@return boolean
function app.png(texture, path) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "list",
            move |vm, mc, (): ()| {
                let (tx, rx) = sync_channel::<Vec<String>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppList(tx))));
                let files = rx.recv().unwrap_or_default();
                let mut t = vm.raw_table();
                for (i, f) in files.iter().enumerate() {
                    t.set(i as i64 + 1, f.to_string());
                }
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: every file in the app being edited",
            "
---@return table
function app.list() end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "reload",
            move |_, _, (): ()| {
                let (tx, rx) = sync_channel::<bool>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppReload(tx))));
                match rx.recv() {
                    Ok(o) => Ok(Value::Bool(o)),
                    Err(_) => Ok(Value::Bool(false)),
                }
            },
            "overlay only: reload the app being edited, so a written file takes effect",
            "
---@return boolean
function app.reload() end",
            &mut app
        );

        // The 3D editing half: look into the app's scene rather than its files.
        let pitcher = main_pitcher.clone();
        lua_lib!(
            "pick",
            move |vm, mc, (own, x, y): (Option<Value>, Option<Value>, Option<Value>)| {
                let own = matches!(own, Some(Value::Bool(true)));
                let at = match (x.as_ref().and_then(value_f32), y.as_ref().and_then(value_f32)) {
                    (Some(x), Some(y)) => Some((x, y)),
                    _ => None,
                };
                let (tx, rx) = sync_channel::<Option<PickOut>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppPick(own, at, tx))));
                let Ok(Some(hit)) = rx.recv() else {
                    return Ok(Value::Nil);
                };
                let mut t = vm.raw_table();
                t.set("kind", hit.kind);
                t.set("id", hit.id as i64);
                t.set("asset", hit.asset);
                t.set("cx", hit.cell[0] as i64);
                t.set("cy", hit.cell[1] as i64);
                t.set("cz", hit.cell[2] as i64);
                t.set("dist", hit.dist as f64);
                t.set("x", hit.point[0] as f64);
                t.set("y", hit.point[1] as f64);
                t.set("z", hit.point[2] as f64);
                t.set("nx", hit.normal[0] as f64);
                t.set("ny", hit.normal[1] as f64);
                t.set("nz", hit.normal[2] as f64);
                if let Some((lo, hi)) = hit.bounds {
                    t.set("x0", lo[0] as f64);
                    t.set("y0", lo[1] as f64);
                    t.set("z0", lo[2] as f64);
                    t.set("x1", hi[0] as f64);
                    t.set("y1", hi[1] as f64);
                    t.set("z1", hi[2] as f64);
                }
                if let Some((name, u, v, w, h, c)) = hit.tex {
                    t.set("tex", name);
                    t.set("u", u as i64);
                    t.set("v", v as i64);
                    t.set("tw", w as i64);
                    t.set("th", h as i64);
                    t.set("color", hex_rgba(c));
                }
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: what's under the pointer — the entity or tile, and the texture and texel it shows",
            "
---@param own? boolean pick the overlay's own 3D instead of the app's scene
---@param x? number screen x 0..1 (default: the pointer)
---@param y? number screen y 0..1
---@return {kind:string,id:integer,asset:string,cx:integer,cy:integer,cz:integer,dist:number,x:number,y:number,z:number,nx:number,ny:number,nz:number,x0:number,y0:number,z0:number,x1:number,y1:number,z1:number,tex:string?,u:integer?,v:integer?,tw:integer?,th:integer?,color:string?}|nil
function app.pick(own, x, y) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "hide",
            move |_, _, (hit, on): (Value, Option<Value>)| {
                let req = mesh_in(&hit)?;
                let on = !matches!(on, Some(Value::Bool(false)));
                let (tx, rx) = sync_channel::<bool>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppHide(req, on, tx))));
                Ok(Value::Bool(rx.recv().unwrap_or(false)))
            },
            "overlay only: hide (or show again) a tile or entity app.pick returned, while it's edited",
            "
---@param hit table what app.pick returned
---@param on? boolean false shows it again
---@return boolean
function app.hide(hit, on) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "mesh",
            move |vm, mc, hit: Value| {
                let req = mesh_in(&hit)?;
                let (tx, rx) = sync_channel::<Result<MeshOut, String>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppMesh(req, tx))));
                let m = match rx.recv() {
                    Ok(Ok(m)) => m,
                    Ok(Err(e)) => {
                        let mut t = vm.raw_table();
                        t.set("err", e);
                        return Ok(vm.wrap_table(mc, t));
                    }
                    Err(_) => return Ok(Value::Nil),
                };
                let p: Vec<Value> = m
                    .pos
                    .iter()
                    .map(|p| vm.table_from_array(mc, [p[0] as f64, p[1] as f64, p[2] as f64]))
                    .collect();
                let u: Vec<Value> = m
                    .uv
                    .iter()
                    .map(|p| vm.table_from_array(mc, [p[0] as f64, p[1] as f64]))
                    .collect();
                let mut t = vm.raw_table();
                t.set("kind", m.kind);
                t.set("asset", m.asset);
                t.set("source", m.source);
                if let Some(path) = m.path {
                    t.set("path", path);
                }
                t.set("x", m.origin[0] as f64);
                t.set("y", m.origin[1] as f64);
                t.set("z", m.origin[2] as f64);
                t.set("rz", m.rz as f64);
                t.set("scale", m.scale as f64);
                t.set("p", vm.table_from_array(mc, p));
                t.set("u", vm.table_from_array(mc, u));
                t.set("t", vm.table_from_array(mc, m.tex));
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: the triangles of a tile or entity app.pick returned, with their textures, for editing",
            "
---@param hit table what app.pick returned
---@return {kind:string, asset:string, source:string, path:string?, x:number, y:number, z:number, rz:number, scale:number, p:number[3][], u:number[2][], t:string[], err:string?}|nil
function app.mesh(hit) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "pause",
            move |_, _, on: Option<Value>| {
                let on = !matches!(on, Some(Value::Bool(false)));
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppPause(on))));
                Ok(Value::Nil)
            },
            "overlay only: freeze (or with false, resume) the app's loop; closing the overlay resumes it",
            "
---@param on? boolean
function app.pause(on) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "drive",
            move |_, _, on: Option<Value>| {
                let on = !matches!(on, Some(Value::Bool(false)));
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppDrive(on))));
                Ok(Value::Nil)
            },
            "overlay only: draw the scene through this overlay's cam (or with false, the app's again); closing the overlay gives the view back",
            "
---@param on? boolean
function app.drive(on) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "view",
            move |vm, mc, (): ()| {
                let (tx, rx) = sync_channel::<Option<CamOut>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppCam(tx))));
                let Ok(Some(c)) = rx.recv() else {
                    return Ok(Value::Nil);
                };
                let mut t = vm.raw_table();
                t.set("x", c.pos[0] as f64);
                t.set("y", c.pos[1] as f64);
                t.set("z", c.pos[2] as f64);
                t.set("yaw", c.rot[0] as f64);
                t.set("pitch", c.rot[1] as f64);
                t.set("ox", c.ray_origin[0] as f64);
                t.set("oy", c.ray_origin[1] as f64);
                t.set("oz", c.ray_origin[2] as f64);
                t.set("dx", c.ray_dir[0] as f64);
                t.set("dy", c.ray_dir[1] as f64);
                t.set("dz", c.ray_dir[2] as f64);
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: the app's camera (x,y,z,yaw,pitch) and the pointer ray (ox..oz, dx..dz)",
            "
---@return table|nil
function app.view() end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "colors",
            move |vm, mc, name: String| {
                let (tx, rx) = sync_channel::<Vec<[u8; 4]>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppColors(name, tx))));
                let cols = rx.recv().unwrap_or_default();
                let mut t = vm.raw_table();
                for (i, c) in cols.iter().enumerate() {
                    t.set(i as i64 + 1, hex_rgba(*c));
                }
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: a texture's most common colours (rrggbbaa), most frequent first",
            "
---@param texture string
---@return string[]
function app.colors(texture) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "source",
            move |vm, mc, name: String| {
                let (tx, rx) = sync_channel::<Result<String, String>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppSource(name, tx))));
                let r = rx.recv().unwrap_or_else(|_| Err("no reply".into()));
                let mut t = vm.raw_table();
                match r {
                    Ok(path) => {
                        t.set("ok", true);
                        t.set("path", path);
                    }
                    Err(e) => {
                        t.set("ok", false);
                        t.set("err", e);
                    }
                }
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: the app's image file a texture was loaded from, if it can be saved back there",
            "
---@param texture string
---@return {ok:boolean, path:string?, err:string?}
function app.source(texture) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "save",
            move |vm, mc, name: String| {
                let (tx, rx) = sync_channel::<Result<String, String>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppSave(name, tx))));
                let r = rx.recv().unwrap_or_else(|_| Err("no reply".into()));
                let mut t = vm.raw_table();
                match r {
                    Ok(path) => {
                        t.set("ok", true);
                        t.set("path", path);
                    }
                    Err(e) => {
                        t.set("ok", false);
                        t.set("err", e);
                    }
                }
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: write a texture's current pixels back into the app's image file it came from",
            "
---@param texture string
---@return {ok:boolean, path:string?, err:string?}
function app.save(texture) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "info",
            move |vm, mc, (): ()| {
                let (tx, rx) = sync_channel::<Option<AppInfoOut>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppInfo(tx))));
                let Ok(Some(i)) = rx.recv() else {
                    return Ok(Value::Nil);
                };
                let mut t = vm.raw_table();
                t.set("dir", i.dir);
                t.set("packed", i.packed);
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: the edited app's folder, and whether it's packed (nothing can be saved)",
            "
---@return {dir:string, packed:boolean}|nil
function app.info() end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "texinfo",
            move |vm, mc, name: String| {
                let (tx, rx) = sync_channel::<Option<TexInfoOut>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppTexInfo(name, tx))));
                let Ok(Some(i)) = rx.recv() else {
                    return Ok(Value::Nil);
                };
                let mut t = vm.raw_table();
                t.set("w", i.w as i64);
                t.set("h", i.h as i64);
                if i.tile > 0 {
                    t.set("tile", i.tile as i64);
                }
                t.set("new", i.new);
                match i.save {
                    Ok(path) => {
                        t.set("ok", true);
                        t.set("path", path);
                    }
                    Err(e) => {
                        t.set("ok", false);
                        t.set("err", e);
                    }
                }
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: a texture's size, its sheet's tile size if it's a tile, and the file it saves into",
            "
---@param texture string
---@return {w:integer, h:integer, tile:integer?, new:boolean, ok:boolean, path:string?, err:string?}|nil
function app.texinfo(texture) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "newtex",
            move |vm, mc, (ent, name, w, h, mode): (Value, String, Value, Value, Option<String>)| {
                let n = |v: &Value| value_f32(v).unwrap_or(0.).max(0.) as u32;
                let mode = match mode.as_deref() {
                    None => crate::unwrap::Mode::Box,
                    Some(m) => crate::unwrap::Mode::parse(m)
                        .ok_or_else(|| static_err("newtex: mode is \"box\" or \"flat\""))?,
                };
                let req = NewTexIn {
                    ent: value_f32(&ent).unwrap_or(-1.) as i64 as u64,
                    name,
                    w: n(&w),
                    h: n(&h),
                    mode,
                };
                let (tx, rx) = sync_channel::<Result<NewTexOut, String>>(0);
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppNewTex(Box::new(req), tx))));
                let r = rx.recv().unwrap_or_else(|_| Err("no reply".into()));
                let mut t = vm.raw_table();
                match r {
                    Ok(o) => {
                        t.set("ok", true);
                        t.set("tex", o.tex);
                        t.set("path", o.path);
                        t.set("unwrapped", o.unwrapped);
                        t.set("charts", o.charts as i64);
                    }
                    Err(e) => {
                        t.set("ok", false);
                        t.set("err", e);
                    }
                }
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: give an entity a fresh image, unwrapping its mesh onto it; saved as a new file",
            "
---@param ent integer entity id (app.pick's id)
---@param name string a-z, 0-9 and _
---@param w integer
---@param h integer
---@param mode? string \"box\" (default) or \"flat\"
---@return {ok:boolean, tex:string?, path:string?, unwrapped:boolean?, charts:integer?, err:string?}
function app.newtex(ent, name, w, h, mode) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "project",
            move |vm, mc, (x, y, z): (f64, f64, f64)| {
                let (tx, rx) = sync_channel::<Option<[f32; 3]>>(0);
                lua_err!(pitcher.send((
                    bundle_id,
                    MainCommmand::AppProject([x as f32, y as f32, z as f32], tx)
                )));
                let Ok(Some(p)) = rx.recv() else {
                    return Ok(Value::Nil);
                };
                let mut t = vm.raw_table();
                t.set("x", p[0] as f64);
                t.set("y", p[1] as f64);
                t.set("depth", p[2] as f64);
                Ok(vm.wrap_table(mc, t))
            },
            "overlay only: a world point's screen position (0..1) and depth, or nil behind the camera",
            "
---@return {x:number,y:number,depth:number}|nil
function app.project(x, y, z) end",
            &mut app
        );

        let pitcher = main_pitcher.clone();
        lua_lib!(
            "close",
            move |_, _, (): ()| {
                lua_err!(pitcher.send((bundle_id, MainCommmand::AppClose)));
                Ok(Value::Nil)
            },
            "overlay only: close this overlay (resuming the app if nothing else holds it)",
            "
function app.close() end",
            &mut app
        );

        vm_init
            .globals
            .borrow_mut(mc_in)
            .set("app", vm_init.wrap_table(mc_in, app));
    }

    COMMAND_DOCS.with(|d| *d.borrow_mut() = command_map.clone());

    vm_init.build_and_run(
        mc_in,
        Some("patch"),
        "
        add=table.insert 
        del=table.remove 
        --print=cout 
        fill = function(...) gui:fill(...) end
        rect = function(...) gui:rect(...) end
        rrect = function(...) gui:rrect(...) end
        line = function(...) gui:line(...) end
        text = function(...) gui:text(...) end
        img = function(...) gui:img(...) end
        clr = function(...) gui:clr(...) end
        pixel = function(...) gui:pixel(...) end
        main = function() end
        loop = function() end
        print('it works?')
        ",
        &mut compiler,
    )?;

    Ok(())
}

/** Error dumping helper */
fn res<E>(target: &str, r: Result<(), E>, loggy: &Sender<(LogType, String)>)
where
    E: std::error::Error,
{
    match r {
        Err(err) => {
            if let Err(_) = loggy.send((
                LogType::LuaSysError,
                format!(
                    "🔴lua::problem setting default lua function {}, {}",
                    target, err
                ),
            )) {}
        }
        _ => {}
    }
}

/** core game reset, drop all resources including lua */
pub fn hard_reset(core: &mut Core) {
    println!("{} {} ", "[ reset ]".on_bright_blue(), "hard reset");
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
    println!("{} {} ", "[ reset ]".on_bright_blue(), "soft reset");
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

pub fn load_from_string(core: &mut Core, sub_command: Option<&str>) {
    load_app(core, sub_command, None, None, None);
}

/** Load an empty game state or bundle for issuing commands */
pub fn load_empty(core: &mut Core) {
    // skip_logo: bool
    let bundle = core.bundle_manager.make_bundle(None, None, None);
    let resources = core.gui.make_morsel();
    let world_sender = core.world.make(bundle.id, core.pitcher.clone());
    bundle.pool = Some(core.gui.make_shared_pool());
    let shared = bundle.pool.clone().unwrap();

    bundle.lua_ctx_handle = Some(bundle.lua.start(
        bundle.id,
        shared,
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
        let header = block_on(crate::asset::unpack(
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
            #[cfg(feature = "audio")]
            &core.singer,
            &mut core.loggy,
            core.global.debug,
        ));
        if let Some((chip_id, monitor_id)) = header {
            crate::root::apply_chip(
                &mut core.global,
                #[cfg(feature = "headed")]
                &mut core.gfx,
                chip_id,
            );
            crate::root::apply_monitor(&mut core.global, monitor_id);
        }
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
pub fn load_app(
    core: &mut Core,
    game_path_in: Option<&str>,
    payload: Option<Vec<u8>>,
    bundle_in: Option<u8>,
    bundle_relations: Option<(u8, bool)>,
) -> Result<(), P64Error> {
    async_load_app(core, game_path_in, payload, bundle_in, bundle_relations, false)
        .block_on()
        .map(|_id| ())
}

/// Close every overlay and drop what they owned outside the bundle manager.
///
/// Entities are the reason this exists rather than calling `close_overlays` directly:
/// they live in `EntManager`, so tearing the bundle down alone left them behind — kept
/// alive, kept drawn, and walked every frame by a bundle that no longer existed.
pub fn close_overlays(core: &mut Core) -> usize {
    let ids = core.bundle_manager.close_overlays();
    if !ids.is_empty() {
        core.unhide_all();
    }
    for id in &ids {
        core.release_view(*id);
        core.ent_manager.reset_by_bundle(*id);
        core.world.destroy(*id);
    }
    ids.len()
}

/// Close one overlay and drop what it drew.
pub fn close_overlay(core: &mut Core, id: u8) -> bool {
    if !core.bundle_manager.close_overlay(id) {
        return false;
    }
    core.unhide_all();
    core.release_view(id);
    core.ent_manager.reset_by_bundle(id);
    core.world.destroy(id);
    true
}

/// Directory name of the built-in paint overlay (`src/builtin.rs`).
pub const PAINT_OVERLAY: &str = "builtin:paint";
/// Directory name of the built-in model editor overlay (`src/builtin.rs`).
pub const MODEL_OVERLAY: &str = "builtin:model";

/// Ctrl+P: open the paint overlay over the running app, or close it if it's
/// up. Refused while the app has `attr{lock=true}` — locking out the engine's
/// own surfaces is what a lock is for.
pub fn toggle_paint_overlay(core: &mut Core) {
    toggle_builtin_overlay(
        core,
        PAINT_OVERLAY,
        "the image editor isn't built into this chor (it needs the studio feature, and github.com/Auxnon/image-chor at build time)",
    );
}

/// Ctrl+M: the model editor overlay (github.com/Auxnon/model-chor), the same
/// way. It takes the view (`app.drive`) starting from the app's camera, so the
/// app's camera comes back when it closes.
pub fn toggle_model_overlay(core: &mut Core) {
    toggle_builtin_overlay(
        core,
        MODEL_OVERLAY,
        "the model editor isn't built into this chor (it needs the studio feature, and github.com/Auxnon/model-chor at build time)",
    );
}

fn toggle_builtin_overlay(core: &mut Core, dir: &str, missing: &str) {
    if let Some(id) = core.bundle_manager.find_by_directory(dir) {
        close_overlay(core, id);
        return;
    }
    if core.global.locked {
        return;
    }
    if core.bundle_manager.edit_target().is_none() {
        return; // nothing to edit
    }
    if crate::builtin::scripts(dir).is_none() {
        core.loggy.log(LogType::ConfigError, missing);
        return;
    }
    if let Err(e) = load_overlay(core, dir) {
        core.loggy
            .log(LogType::ConfigError, &format!("{} overlay failed: {}", dir, e));
    }
}

/// Bring an app up as an **overlay**: an editing surface over the running app, which
/// owns input, draws on its own gui layer, and gets the privileged `app.*` table.
///
/// The only way to make one, and reachable only from the console and `--overlay` — an
/// app's own Lua has no path here (its `over()` makes a plain child bundle). Keeping
/// the whole sequence in one function is the point: the mark has to be set before the
/// VM is built, and two call sites hand-rolling that is how it drifts.
pub fn load_overlay(core: &mut Core, path: &str) -> Result<u8, P64Error> {
    // The id comes back from the load. It used to be predicted by reading the id
    // counter beforehand, which only held while ids were handed out in order and
    // never reused; they are now allocated from the lowest free slot.
    async_load_app(core, Some(path), None, None, None, true).block_on()
}
pub async fn load_app_and_log(
    core: &mut Core,
    game_path_in: Option<&str>,
    payload: Option<Vec<u8>>,
    bundle_in: Option<u8>,
    bundle_relations: Option<(u8, bool)>,
) {
    if let Err(e) = load_app(core, game_path_in, payload, bundle_in, bundle_relations) {
        let res: String = format!("{}", e);
        core.loggy.log(LogType::LuaError, &res);
    }
}

async fn async_load_app(
    core: &mut Core,
    game_path_in: Option<&str>,
    payload: Option<Vec<u8>>,
    bundle_in: Option<u8>,
    bundle_relations: Option<(u8, bool)>,
    as_overlay: bool,
) -> Result<u8, P64Error> {
    println!(
        "{} {}",
        "loading from script".on_green(),
        game_path_in.unwrap_or("~")
    );
    let bundle = match bundle_in {
        Some(b) => {
            let bun = core.bundle_manager.bundles.get_mut(&b).unwrap();
            bun
        }
        None => core
            .bundle_manager
            .make_bundle(game_path_in, bundle_relations, game_path_in),
    };

    bundle.pool = Some(core.gui.make_shared_pool());

    // Marked here, before the Lua thread starts, because the `app.*` table is decided
    // at VM build time — marking after the load (as the console command used to) left
    // an overlay's first frames running without the tools it was opened for. A reload
    // of an already-marked overlay keeps its mark, and nothing else ever sets one.
    if as_overlay {
        bundle.overlay = true;
    }
    let privileged = bundle.overlay;

    let bundle_id = bundle.id;
    let resources = core.gui.make_morsel();
    let world_sender = core.world.make(bundle.id, core.pitcher.clone());

    // App (re)load: drop the `attr` state the previous app declared, so a new game
    // starts from defaults instead of inheriting it. The console lock is the one
    // that bites — the boot fallback (`nil`) locks the console, so every game
    // loaded after it used to inherit the lock and stay unreachable. This only
    // clears the lock; whether the console is *open* is left exactly as it was
    // (an open console captures all keyboard input).
    //
    // None of this applies to an overlay (`privileged`): it loads *over* the app,
    // and resetting the app's attrs dropped its lock, while the sound reset
    // wiped every instrument the app had registered — opening an editor
    // silenced the game for good.
    if !privileged {
        core.global.clean_app_attrs();

        // App (re)load: clear any instruments/samples a previous run registered so
        // a since-removed `smpl`/`instr` doesn't keep playing the stale definition.
        #[cfg(feature = "audio")]
        let _ = core.singer.send(SoundCommand::Reset);
        // MIDI hardware outlives a reload (like the audio stream), but the new app
        // shouldn't inherit the old one's queued events or held notes.
        #[cfg(all(feature = "midi", not(target_arch = "wasm32")))]
        crate::midi::reset(&core.singer);
    }

    let shared = bundle.pool.clone().unwrap();
    bundle.lua_ctx_handle = Some(bundle.lua.start(
        bundle_id,
        shared,
        resources,
        world_sender,
        core.pitcher.clone(),
        core.loggy.make_sender(),
        #[cfg(feature = "audio")]
        core.singer.clone(),
        core.global.debug,
        privileged,
    ));

    let debug = core.global.debug;

    // TODO ensure this is reset before load
    // core.tex_manager.reset();

    // if we get a path and it's a file, it needs to be unpacked, if it's a custom directoty we walk it, otherwise walk the local directory
    // The first-line `--! 0xNN` chip+monitor header (see guide/mon.md), if any
    // loaded script declared one — applied below, same effect as calling
    // chip()/mon() once before main() runs.
    let builtin = bundle.get_directory().and_then(crate::builtin::scripts);
    let header: Option<(u8, u8)> = match bundle.get_directory() {
        // Engine-embedded apps (the Ctrl+P paint overlay): scripts compiled into
        // the binary, so an overlay's code never comes from the app it edits or
        // from whatever directory chor happens to run in.
        Some(_) if builtin.is_some() => {
            for (name, src) in builtin.unwrap_or(&[]) {
                let mut reader: &[u8] = src.as_bytes();
                if let Err(e) = bundle.lua.async_load(name.to_string(), &mut reader) {
                    core.loggy
                        .log(LogType::LuaError, &format!("!!{}: {}", name, e));
                }
            }
            None
        }
        Some(s) => match payload {
            Some(p) => {
                let h = crate::asset::unpack(
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
                    #[cfg(feature = "audio")]
                    &core.singer,
                    &mut core.loggy,
                    debug,
                )
                .await;
                // println!("unpacked");
                h
            }
            None => {
                let mut path = crate::asset::resolve_app_path(s.as_ref());
                if path.is_dir() {
                    let asset_items = crate::asset::get_asset_items(&path, &mut core.loggy)?;
                    let script_items = crate::asset::get_script_items(&path, &mut core.loggy)?;

                    #[cfg(feature = "audio")]
                    crate::asset::load_sounds_from_dir(&path, &core.singer, &mut core.loggy);

                    let (_, h) = crate::asset::walk_files(
                        #[cfg(feature = "headed")]
                        Some(&core.gfx.device),
                        #[cfg(feature = "headed")]
                        &mut core.tex_manager,
                        &mut core.model_manager,
                        true,
                        &mut core.world,
                        bundle_id,
                        &bundle.lua,
                        &asset_items,
                        &script_items,
                        // Sounds already loaded above via load_sounds_from_dir;
                        // walk_files's packable-list return is unused on load.
                        &[],
                        &mut core.loggy,
                        debug,
                    );
                    h
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
                                let buff = crate::file_util::get_file_buffer_from_path(path)?;

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
                                    #[cfg(feature = "audio")]
                                    &core.singer,
                                    &mut core.loggy,
                                    debug,
                                )
                                .await
                            } else {
                                return Err(P64Error::IoNotFileOrDir(s.into()));
                            }
                        }
                        None => {
                            return Err(P64Error::IoNotFileOrDir(s.into()));
                        }
                    }
                }
            }
        },
        None => {
            let path = crate::asset::determine_path(None);
            let asset_items = crate::asset::get_asset_items(&path, &mut core.loggy)?;
            let script_items = crate::asset::get_script_items(&path, &mut core.loggy)?;

            #[cfg(feature = "audio")]
            crate::asset::load_sounds_from_dir(&path, &core.singer, &mut core.loggy);

            let (_, h) = crate::asset::walk_files(
                #[cfg(feature = "headed")]
                Some(&core.gfx.device),
                #[cfg(feature = "headed")]
                &mut core.tex_manager,
                &mut core.model_manager,
                true,
                &mut core.world,
                bundle_id,
                &bundle.lua,
                &asset_items,
                &script_items,
                &[],
                &mut core.loggy,
                debug,
            );
            h
        }
    };
    if let Some((chip_id, monitor_id)) = header {
        crate::root::apply_chip(
            &mut core.global,
            #[cfg(feature = "headed")]
            &mut core.gfx,
            chip_id,
        );
        crate::root::apply_monitor(&mut core.global, monitor_id);
    }
    #[cfg(feature = "headed")]
    core.tex_manager
        .refinalize(&core.gfx.queue, &core.gfx.master_texture);
    // DEV  TODO
    // for e in &mut entity_manager.entities {
    //     e.hot_reload();
    // }
    let dir = match bundle.get_directory() {
        Some(s) => s,
        None => "_",
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
    bundle.call_main()?;
    Ok(bundle_id)
}

/** reset and load previously loaded game, OR reload the binary binded game if compiled with it*/
pub fn reload(core: &mut Core, bundle_id: u8) {
    if soft_reset(core, bundle_id) {
        println!("reload from current bundle");
        block_on(load_app_and_log(core, None, None, Some(bundle_id), None));
    } else {
        #[cfg(feature = "include_auto")]
        {
            core.loggy
                .log(LogType::Config, "auto loading included bytes");
            let payload = include_bytes!("../auto.game.png").to_vec();
            println!("auto load bin from reload command");
            block_on(load_app_and_log(
                core,
                Some("INCLUDE_AUTO"),
                Some(payload),
                None,
                None,
            ));
        }
        #[cfg(not(feature = "include_auto"))]
        {
            println!("reload into empty bundle");
            block_on(load_app_and_log(core, None, None, None, None));
        }
    }
}

// static KEYS: [String; 256] = ["1", "2", "3", "4", "5", "6", "7", "8", "9", "0", "a", "b",
// "c", "d", "e", "f", "g", "h", "i", "j", "k", "l", "m", "n", "o", "p", "q", "r", "s", "t", "u",
// "v", "w", "x", "y", "z", "escape", "f1", "f2", "f3", "f4", "f5", "f6", "f7", "f8", "f9",
// "f10", "f11", "f12", "f13","f14","f15", "snap","snapshot","dele"];
pub(crate) fn key_match(key: String) -> usize {
    // KeyCode::from_str(&key).unwrap() as usize
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
        "insert" => 64,
        "home" => 65,
        "del" => 66,
        "end" => 67,
        "pagedown" => 68,
        "pageup" => 69,
        "left" => 70,
        "up" => 71,
        "right" => 72,
        "down" => 73,
        "back" => 74,

        "return" | "enter" => 75,
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
        "lbracket" => 116,
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
        "rbracket" => 137,
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

        // "space" => KeyCode::Space,
        // "lctrl" => KeyCode::LControl,
        // "rctrl" => KeyCode::RControl,
        // Left/right modifier names collapse onto the same slots as the bare ones.
        // `bit_check` never distinguishes sides, so these used to name indices that
        // nothing on any code path ever wrote: `key("lctrl")` was permanently false,
        // silently, which is a worse answer than "no such key".
        "alt" | "lalt" | "ralt" => 247,
        "ctrl" | "control" | "lctrl" | "rctrl" => 248,
        "shift" | "lshift" | "rshift" => 249,
        "super" | "win" | "lwin" | "rwin" => 250,

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
    SetImg(String, RgbaImage, SyncSender<()>),
    /// (pos, rot, ortho half-height: 0 is perspective)
    Cam(Option<glam::Vec3>, Option<glam::Vec2>, Option<f32>),
    /// The one light `lum{}` sets — (dir, sun rgb, flat ambient, sky rgb,
    /// ground rgb, shape, pos, range, angle), each optional so a call can set
    /// just one aspect. sky+ground present => hemisphere ambient (sun only);
    /// else the flat scalar is used. `shape`: 0=sun (default), 1=cone,
    /// 2=sphere — see `guide/lum.md`.
    Light(
        Option<glam::Vec3>,
        Option<glam::Vec3>,
        Option<f32>,
        Option<glam::Vec3>,
        Option<glam::Vec3>,
        Option<u8>,
        Option<glam::Vec3>,
        Option<f32>,
        Option<f32>,
    ),
    /// Distance fog: rgb + w = far distance (w=0 disables).
    Fog(glam::Vec4),
    MouseGrab(bool),
    /// Enable/disable the shadow pass — see `guide/shdw.md`.
    /// on, then optional map size (texels) and sun range (tiles)
    Shdw(bool, Option<f32>, Option<f32>),
    /// Enable/disable Gouraud (per-vertex) shading — see `guide/gour.md`.
    Gour(bool),
    /// Graphics chip preset: 0=R00 (modern), 1=R43 (N64-ish), 2=R30 (PS1-ish).
    /// See `guide/chip.md`.
    SetChip(u8),
    GetChip(SyncSender<u8>),
    /// Display monitor preset: 0=LCD, 1=Slot (slot-mask CRT), 2=Grille
    /// (aperture-grille CRT). See `guide/mon.md`.
    SetMonitor(u8),
    GetMonitor(SyncSender<u8>),
    /// Pairwise collision test between two entity snapshots (`ent:hit(ent2)`'s
    /// backing call — see `guide/hit.md`). `Some((nx,ny,nz,depth))` if they
    /// overlap, `None` otherwise. Takes snapshots rather than ids because the
    /// two entities may not even be in the same bundle as the caller.
    HitPair(
        UserDataWrapper,
        UserDataWrapper,
        SyncSender<Option<(f32, f32, f32, f32)>>,
    ),
    /// The O(n²)-avoided batch query behind `hit.all()`: every currently-
    /// overlapping ent-vs-ent pair in the sender's bundle.
    HitAll(SyncSender<Vec<(u64, u64, f32, f32, f32, f32)>>),
    /// Every solid tile entity `id`'s collider currently overlaps — backs
    /// `hit.cell(id)`.
    HitCell(u64, SyncSender<Vec<(i32, i32, i32, f32, f32, f32, f32)>>),
    /// Every hit between entity `id`'s collider and any *other* entity's
    /// tile grid (see `src/tile_grid.rs`, `guide/hit.md`) — backs
    /// `hit.grid(id)`.
    HitGrid(
        u64,
        SyncSender<Vec<(u64, i32, i32, i32, f32, f32, f32, f32)>>,
    ),
    Make(Vec<String>, SyncSender<u8>),
    Anim(String, Vec<String>, u32),
    // Spawn(Arc<std::sync::Mutex<LuaEnt>>),
    Spawn(UserDataWrapper),
    Group(u64, u64),
    Model(Box<ModelPacket>),
    ListModel(String, Option<u8>, SyncSender<Vec<String>>),
    Globals(Vec<(String, ValueMap)>),
    GetGlobal(SyncSender<GlobalMap>),
    AsyncError(String),
    LoopComplete(BundleMutations),
    Reload(),
    BundleDropped(BundleResources),
    Load(String),
    Subload(String, bool),
    WorldSync(Vec<Chunk>, bool),
    /// `tileset(...)`: tiles that mesh together on the dual grid.
    Tileset(crate::tileset::TilesetDef),
    /// Microphone capture. `Some(id)` starts a capture of `secs` seconds into
    /// sample slot `id`, replying whether it actually started (false = no input
    /// device, or one is already running). `None` is a **query only** — it never
    /// opens the mic, and replies whether a capture is in flight. The input stream
    /// has to be built and owned on the main thread (cpal streams aren't Send),
    /// hence the round trip.
    MicRecord(Option<usize>, f32, SyncSender<bool>),
    Null(),
    Stats(),
    Read(String, SyncSender<Option<String>>),
    Write(String, String, SyncSender<bool>),
    /// `glb(path, data)`: path, mesh, and the textures to embed — one, or
    /// one per `tex` index of the mesh, baked into a single image.
    GlbWrite(
        Box<(String, crate::gltf_io::MeshData, Vec<String>)>,
        SyncSender<bool>,
    ),
    /// `glb(path)`: the file's triangles, or None if it can't be read.
    GlbRead(String, SyncSender<Option<crate::gltf_io::MeshData>>),
    /// The `app.*` family: an overlay reaching into the app it edits. The main thread
    /// re-checks that the sender really is an engine-marked overlay before acting on
    /// any of these — the natives are only installed into overlay VMs, but a
    /// permission this sharp shouldn't rest on that alone.
    AppRead(String, SyncSender<Option<String>>),
    AppWrite(String, String, SyncSender<bool>),
    AppList(SyncSender<Vec<String>>),
    AppReload(SyncSender<bool>),
    /// `app.pick(own, x, y)`: what's under the pointer (or screen point x, y) —
    /// in the app's scene, or in the overlay's own 3D (`own`).
    AppPick(bool, Option<(f32, f32)>, SyncSender<Option<PickOut>>),
    /// `app.png(texture, path)`: a texture written out as a png in the app.
    AppPng(String, String, SyncSender<bool>),
    /// `app.mesh(hit)`: the triangles of a picked tile or entity.
    AppMesh(MeshIn, SyncSender<Result<MeshOut, String>>),
    /// `app.hide(hit, on)`: hide or show a picked tile or entity.
    AppHide(MeshIn, bool, SyncSender<bool>),
    /// `app.pause(on)`: freeze or resume the app's loop.
    AppPause(bool),
    /// `app.drive(on)`: draw the scene through this overlay's camera.
    AppDrive(bool),
    /// `app.view()`: the camera the scene was drawn with, and the pointer ray.
    AppCam(SyncSender<Option<CamOut>>),
    /// `app.colors(texture)`: its most common colours, most frequent first.
    AppColors(String, SyncSender<Vec<[u8; 4]>>),
    /// `app.project(x, y, z)`: a world point on screen (0..1) and its depth.
    AppProject([f32; 3], SyncSender<Option<[f32; 3]>>),
    /// `app.close()`: an overlay dismissing itself.
    AppClose,
    /// `app.source(texture)`: the app's image file a texture can be saved to
    /// (path relative to the app), or why it can't.
    AppSource(String, SyncSender<Result<String, String>>),
    /// `app.save(texture)`: write a texture's pixels back into that file.
    AppSave(String, SyncSender<Result<String, String>>),
    /// `app.info()`: where the edited app lives and whether its files can be
    /// written at all (a packed game can't).
    AppInfo(SyncSender<Option<AppInfoOut>>),
    /// `app.texinfo(texture)`: its size, the sheet tile it is, its file.
    AppTexInfo(String, SyncSender<Option<TexInfoOut>>),
    /// `app.newtex(ent, name, w, h, mode)`: a fresh image for one entity,
    /// unwrapping its mesh onto it.
    AppNewTex(Box<NewTexIn>, SyncSender<Result<NewTexOut, String>>),
    Copy(String),
    LuaClose(),
    //for testing
    // Meta(crate::gui::ScreenIndex),
    InitBack(Box<(WeakWrapper, WeakWrapper)>),
    Quit(u8),
}

/// An `app.pick` hit, as plain data for the trip back to the VM. Built from
/// `pick::PickHit` on the main thread (which only exists in headed builds).
pub struct PickOut {
    /// `"ent"` or `"tile"`.
    pub kind: &'static str,
    pub id: u64,
    pub asset: String,
    pub cell: [i32; 3],
    pub dist: f32,
    pub point: [f32; 3],
    pub normal: [f32; 3],
    /// Texture name, texel x/y, texture w/h, and the texel's colour.
    pub tex: Option<(String, u32, u32, u32, u32, [u8; 4])>,
    /// The thing's world-space box (min, max).
    pub bounds: Option<([f32; 3], [f32; 3])>,
}

/// The tile or entity an `app.pick` result names, for `app.mesh`/`app.hide`.
fn mesh_in(hit: &Value) -> Result<MeshIn, LuaError> {
    let Value::Table(h) = hit else {
        return Err(static_err("pass the table app.pick returned"));
    };
    let h = h.borrow();
    let int = |k: &str| match h.get_value(&k.into()) {
        Value::Integer(n) => n,
        Value::Number(n) => n as i64,
        _ => 0,
    };
    let ent = matches!(h.get_value(&"kind".into()), Value::String(s) if s.to_string() == "ent");
    Ok(MeshIn {
        ent,
        id: int("id") as u64,
        cell: [int("cx") as i32, int("cy") as i32, int("cz") as i32],
    })
}

/// `app.mesh(hit)`'s request: an entity id, or a tile cell.
pub struct MeshIn {
    pub ent: bool,
    pub id: u64,
    pub cell: [i32; 3],
}

/// `app.mesh(hit)`: the thing's triangles in its own model space, three
/// corners each, with a texture name per triangle and a uv per corner in that
/// texture's own 0..1. World = `origin` + (turned `rz` about z) model space.
#[derive(Default)]
pub struct MeshOut {
    /// `"model"`, `"cube"` (a tile that's only a texture) or `"sprite"`.
    pub kind: &'static str,
    pub asset: String,
    /// `"file"` (a glb/gltf, at `path`), `"code"` (`mod()`), `"texture"`
    /// (cube or sprite), `"packed"` (a file inside a packed game) or
    /// `"engine"` (the built-in cube and plane).
    pub source: &'static str,
    /// The file, relative to the app, for `"file"`.
    pub path: Option<String>,
    pub origin: [f32; 3],
    pub rz: f32,
    pub scale: f32,
    pub pos: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub tex: Vec<String>,
}

/// `app.info()`.
pub struct AppInfoOut {
    pub dir: String,
    /// Loaded from a packed `.game.png` (or anything but a folder): nothing to
    /// save into.
    pub packed: bool,
}

/// `app.texinfo(texture)`.
pub struct TexInfoOut {
    pub w: u32,
    pub h: u32,
    /// Its sheet's tile size if it's one tile of a sheet, else 0.
    pub tile: u32,
    /// The file it saves into (relative to the app), or why it can't.
    pub save: Result<String, String>,
    /// Made by NEW and not written yet.
    pub new: bool,
}

/// `app.newtex(...)`'s request.
pub struct NewTexIn {
    pub ent: u64,
    pub name: String,
    pub w: u32,
    pub h: u32,
    pub mode: crate::unwrap::Mode,
}

/// `app.newtex(...)`'s result.
pub struct NewTexOut {
    /// The new texture's name (and, for a mesh, the new model's).
    pub tex: String,
    /// The file the first save will create, relative to the app.
    pub path: String,
    /// The mesh was unwrapped (false: a sprite or `tex` cube keeps its mapping).
    pub unwrapped: bool,
    pub charts: usize,
}

/// `app.view()`: camera position, yaw/pitch, and the pointer ray in world units.
pub struct CamOut {
    pub pos: [f32; 3],
    pub rot: [f32; 2],
    pub ray_origin: [f32; 3],
    pub ray_dir: [f32; 3],
}

fn value_f32(v: &Value) -> Option<f32> {
    match v {
        Value::Number(f) => Some(*f as f32),
        Value::Integer(i) => Some(*i as f32),
        _ => None,
    }
}

/// `rrggbbaa`, which every colour argument accepts.
pub fn hex_rgba(c: [u8; 4]) -> String {
    format!("{:02x}{:02x}{:02x}{:02x}", c[0], c[1], c[2], c[3])
}

/// Translate a VM→host command into the serializable worker message for the
/// wasm postMessage bridge (§4b). The worker drains its local MainPacket channel
/// after each dispatched message and posts the results to the main thread.
///
/// Only the fire-and-forget subset is mapped: `Cam`, `Globals`, `LoopComplete`,
/// `AsyncError`, `Spawn`, and `Model`/`Make` (the `mod` native sends a reply
/// channel but never reads it, so they're acked here and forwarded). Blocking
/// request/response commands (GetImg/GetGlobal/Read/Write/Die — they
/// carry a SyncSender that is read) and host-internal
/// ones return `None` and are handled in the read-back phase. Compiled on all
/// targets so a new `MainCommmand` variant or `VmToHost` change is type-checked
/// everywhere, not just on wasm.
#[allow(dead_code)]
pub fn main_command_to_host(cmd: MainCommmand) -> Option<crate::worker_protocol::VmToHost> {
    use crate::worker_protocol::{ValueWire, VmToHost};
    match cmd {
        MainCommmand::Cam(pos, rot, ortho) => Some(VmToHost::Cam {
            pos: pos.map(|v| [v.x, v.y, v.z]),
            rot: rot.map(|v| [v.x, v.y]),
            ortho,
        }),
        MainCommmand::Light(dir, color, ambient, sky, ground, shape, pos, range, angle) => {
            Some(VmToHost::Light {
                dir: dir.map(|v| [v.x, v.y, v.z]),
                color: color.map(|v| [v.x, v.y, v.z]),
                ambient,
                sky: sky.map(|v| [v.x, v.y, v.z]),
                ground: ground.map(|v| [v.x, v.y, v.z]),
                shape,
                pos: pos.map(|v| [v.x, v.y, v.z]),
                range,
                angle,
            })
        }
        MainCommmand::Fog(v) => Some(VmToHost::Fog([v.x, v.y, v.z, v.w])),
        MainCommmand::Tileset(def) => Some(VmToHost::Tileset(def)),
        MainCommmand::MouseGrab(on) => Some(VmToHost::MouseGrab(on)),
        MainCommmand::Globals(table) => Some(VmToHost::Globals(
            table
                .iter()
                .map(|(k, v)| (k.clone(), ValueWire::from(v)))
                .collect(),
        )),
        MainCommmand::LoopComplete(m) => Some(VmToHost::LoopComplete {
            gui: m.gui,
            sky: m.sky,
        }),
        MainCommmand::AsyncError(s) => Some(VmToHost::Error(s)),
        MainCommmand::SetImg(name, img, _tx) => {
            let (w, h) = img.dimensions();
            Some(VmToHost::SetImg {
                name,
                w,
                h,
                px: img.into_raw(),
            })
        }
        MainCommmand::Spawn(wrapper) => wrapper
            .downcast_ref::<crate::lua_ent::LuaEnt, _, _>(|lent| Ok(lent.clone()))
            .ok()
            .map(VmToHost::Spawn),
        MainCommmand::Model(m) => {
            // Nobody waits on the ack (see the `mod` native).
            let _ = m.sender.send(0);
            let ModelPacket {
                asset,
                textures,
                vecs,
                norms,
                inds,
                uvs,
                style,
                ..
            } = *m;
            Some(VmToHost::Model {
                asset,
                textures,
                vecs,
                norms,
                inds,
                uvs,
                style,
            })
        }
        MainCommmand::Make(m, tx) => {
            let _ = tx.send(0);
            Some(VmToHost::Make(m))
        }
        _ => None,
    }
}

pub fn num(x: Value) -> LuaResponse {
    match x {
        Value::Integer(i) => LuaResponse::Integer(i),
        Value::Number(f) => LuaResponse::Number(f),
        Value::String(s) => LuaResponse::String(s),
        _ => LuaResponse::Integer(0),
    }
}

fn i64_to_i32(x: i64) -> i32 {
    if x > i32::MAX as i64 {
        i32::MAX
    } else if x < i32::MIN as i64 {
        i32::MIN
    } else {
        x as i32
    }
}

pub fn numop(x: Option<Value>) -> LuaResponse {
    match x {
        Some(v) => num(v),
        _ => LuaResponse::Integer(0),
    }
}
fn nummold(x: Value) -> NumCouple {
    match x {
        Value::Integer(i) => (true, i as f32),
        Value::Number(f) => (f >= 2., f as f32),
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

fn table_hasher(table: &Table) -> Vec<(String, ValueMap)> {
    let mut data = vec![];
    for (key, val) in table.iter() {
        let str_key = if let Some(str_key) = match key {
            Value::String(s) => Some(s.to_owned()),

            Value::Integer(i) => Some(i.to_string()),
            _ => None,
        } {
            let mapped = match val {
                Value::String(s) => ValueMap::String(s.to_owned()),

                Value::Integer(i) => ValueMap::Integer(i64_to_i32(*i)),
                Value::Number(n) => ValueMap::Float(*n as f32),
                Value::Bool(b) => ValueMap::Bool(*b),
                Value::Table(t) => {
                    ValueMap::Array(
                        t.borrow()
                            .iter()
                            .filter_map(|(k, v)| match v {
                                Value::String(s) => Some(ValueMap::String(s.to_owned())),
                                Value::Integer(i) => Some(ValueMap::Integer(i64_to_i32(*i))),
                                Value::Number(n) => Some(ValueMap::Float(*n as f32)), // TODO inaccurate
                                Value::Bool(b) => Some(ValueMap::Bool(*b)),
                                _ => None,
                            })
                            .collect::<Vec<ValueMap>>(),
                    )
                    // ValueMap::Table(table_hasher(&t, recursion_check + 1))
                }
                _ => ValueMap::Null(),
            };
            data.push((str_key, mapped));
        };
    }
    data
}

/// Read `t[key]` as an ordered list, walking indices 1..n with `getn`. Silt's
/// `Vec<T>` FromLua iterates the table's hashmap, so a list like `q`'s four
/// corners comes back in hash order — a quad's winding scrambles into a bowtie.
/// A bare non-table value is a one-element list (`t="tex"` == `t={"tex"}`).
/// `None` when the key is absent.
#[cfg(feature = "silt")]
fn ordered_list<'v, R>(t: &Table<'v>, key: &str, vm: &VM<'v>, mc: &Mutation<'v>) -> Option<Vec<R>>
where
    R: silt_lua::value::FromLua<'v> + Default,
{
    match t.get_value(&key.into()) {
        Value::Nil => None,
        Value::Table(inner) => {
            let tb = inner.borrow();
            let mut out = Vec::new();
            let mut i = 1;
            while let Some(v) = tb.getn(i) {
                out.push(R::from_lua(v, vm, mc).unwrap_or_default());
                i += 1;
            }
            Some(out)
        }
        v => Some(vec![R::from_lua(&v, vm, mc).unwrap_or_default()]),
    }
}

/// Points from a Lua list of `{x, y, z}` tables or `vec3` values.
fn point_list<'v>(t: &Table<'v>, key: &str) -> Option<Vec<[f32; 3]>> {
    match t.get_value(&key.into()) {
        Value::Table(inner) => {
            let tb = inner.borrow();
            let mut out = Vec::new();
            let mut i = 1;
            while let Some(v) = tb.getn(i) {
                out.push(match v {
                    #[cfg(feature = "silt")]
                    Value::Vec3(p) => [p.x, p.y, p.z],
                    Value::Table(p) => p.borrow().to_array::<f32, 3>(),
                    _ => [0., 0., 0.],
                });
                i += 1;
            }
            Some(out)
        }
        _ => None,
    }
}

/// The polygon (`p` + `f`) and line (`l`) forms shared by `mod()` and `glb()`.
/// None if the table is neither, so the caller falls through to its other forms.
///
/// - polygon: `p` positions, `f` faces as lists of **1-based** indices into `p`
///   (Lua's own convention, unlike `i`), `fu` one uv per face, or `fc` a uv
///   per corner of each face with `ft` a texture name per face
/// - line: `l` segment endpoints in pairs, `lw` width (one, or per segment),
///   `lu` one uv per segment
/// - either: `scale` multiplies positions (see guide/mod.md on precision)
///
/// Alongside the mesh: the textures `ft` names, in the order the mesh's `tex`
/// indices count them (empty without `ft`).
fn mesh_from_table<'v>(
    t: &Table<'v>,
    vm: &VM<'v>,
    mc: &Mutation<'v>,
) -> Option<Result<(crate::gltf_io::MeshData, Vec<String>), String>> {
    let scale = match t.get_value(&"scale".into()) {
        Value::Integer(n) => n as f32,
        Value::Number(n) => n as f32,
        _ => 1.,
    };
    let mut names: Vec<String> = vec![];
    let mut mesh = if let Value::Table(ft) = t.get_value(&"f".into()) {
        let pos = point_list(t, "p").unwrap_or_default();
        let ftb = ft.borrow();
        let mut faces = Vec::new();
        let mut i = 1;
        while let Some(Value::Table(face)) = ftb.getn(i) {
            let fb = face.borrow();
            let mut idx = Vec::new();
            let mut k = 1;
            while let Some(v) = fb.getn(k) {
                let n = match v {
                    Value::Integer(n) => *n,
                    Value::Number(n) => *n as i64,
                    _ => 0,
                };
                if n < 1 {
                    return Some(Err(format!("face {} has index {} (indices start at 1)", i, n)));
                }
                idx.push((n - 1) as u32);
                k += 1;
            }
            faces.push(idx);
            i += 1;
        }
        let fu = ordered_list::<[f32; 2]>(t, "fu", vm, mc).unwrap_or_default();
        // fc: per face, a list of {u, v} for its corners
        let mut fc: Vec<Vec<[f32; 2]>> = vec![];
        if let Value::Table(ct) = t.get_value(&"fc".into()) {
            let ctb = ct.borrow();
            let mut i = 1;
            while let Some(face) = ctb.getn(i) {
                let mut corners = vec![];
                if let Value::Table(face) = face {
                    let fb = face.borrow();
                    let mut k = 1;
                    while let Some(Value::Table(uv)) = fb.getn(k) {
                        corners.push(uv.borrow().to_array::<f32, 2>());
                        k += 1;
                    }
                }
                fc.push(corners);
                i += 1;
            }
        }
        let mut face_tex: Vec<u32> = vec![];
        for name in ordered_list::<String>(t, "ft", vm, mc).unwrap_or_default() {
            let at = match names.iter().position(|n| *n == name) {
                Some(at) => at,
                None => {
                    names.push(name);
                    names.len() - 1
                }
            };
            face_tex.push(at as u32);
        }
        match crate::gltf_io::MeshData::from_polygons(&pos, &faces, &fu, &fc, &face_tex) {
            Ok(m) => m,
            Err(e) => return Some(Err(e)),
        }
    } else if let Some(pts) = point_list(t, "l") {
        let lw = ordered_list::<f32>(t, "lw", vm, mc).unwrap_or_default();
        let lu = ordered_list::<[f32; 2]>(t, "lu", vm, mc).unwrap_or_default();
        crate::gltf_io::MeshData::from_lines(&pts, &lw, &lu)
    } else {
        return None;
    };
    if scale != 1. {
        for p in mesh.pos.iter_mut() {
            *p = [p[0] * scale, p[1] * scale, p[2] * scale];
        }
    }
    Some(Ok((mesh, names)))
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
        // println!("quad_norm {:?}", quad_norm);
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
        // println!("new_quad_norm {:?}", new_quad_norm); //should be [0,1,0]

        // println!(
        //     "original v1dir {:?} v2dir {:?} v3dir {:?} v4dir {:?}",
        //     v1dir, v2dir, v3dir, v4dir
        // );

        // println!("rotmat {:?}", rotmat);
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
        // println!(
        //     "v1dir {:?} v2dir {:?} v3dir {:?} v4dir {:?}",
        //     v1dir, v2dir, v3dir, v4dir
        // );
        // println!(
        //     "v1length {:?} v2length {:?} v3length {:?} v4length {:?}",
        //     v1length, v2length, v3length, v4length
        // );

        let n1 = [0., 0.];
        let n2 = [v1dirx, v1diry];
        let n3 = [n2[0] + v2dirx, n2[1] + v2diry];
        let n4 = [n3[0] + v3dirx, n3[1] + v3diry];
        // println!("n1 {:?} n2 {:?} n3 {:?} n4 {:?}", n1, n2, n3, n4);

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

fn make_err(s: &str) -> LuaError {
    // LuaError:::from(Value::String(piccolo::String::from(s.to_string())))
    // make_static(s)
    LuaError::Custom(s.to_string())
    // LuaError::RuntimeError(s.to_string())
    // return mlua::Error::RuntimeError(s.to_string());
}

// TODO ???
fn static_err<M>(s: M) -> LuaError
where
    M: Display + core::fmt::Debug + Send + Sync + 'static,
{
    // StaticError::Runtime(RuntimeError(Arc::new(anyhow::Error::msg(s))))
    LuaError::Custom(s.to_string())
    // StaticError::Lua(StaticLuaError(s.to_owned()))
}

fn context_err<M>(s: M) -> LuaError
where
    M: Display + core::fmt::Debug + Send + Sync + 'static,
{
    // piccolo::Error::Runtime(RuntimeError(Arc::new(anyhow::Error::msg(s))))
    LuaError::Custom(s.to_string())
}

/** Convert string command into easy to use hashmap */
fn get_com_hash(svec: Vec<&str>, paired: Vec<&str>) -> (Vec<String>, HashMap<String, String>) {
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

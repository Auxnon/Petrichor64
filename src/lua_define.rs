use crate::{
    ent::Ent, ent_manager, ent_master, global::Global, lua_ent::LuaEnt, switch_board::SwitchBoard,
};
use lazy_static::lazy_static;
use mlua::{Function, Lua, Scope, UserData, UserDataMethods};
use once_cell::sync::OnceCell;
use parking_lot::{lock_api::RawMutexFair, Mutex, MutexGuard, RwLock};
use std::{
    cell::RefCell,
    collections::HashMap,
    fs,
    path::Path,
    process::exit,
    rc::Rc,
    sync::{
        mpsc::{channel, sync_channel, Receiver, SendError, Sender, SyncSender},
        Arc,
    },
    thread,
};

pub struct LuaCore {
    // pub lua: Mutex<mlua::Lua>,
    to_lua_tx: Sender<(
        String,
        String,
        Option<LuaEnt>,
        SyncSender<(Option<String>, Option<LuaEnt>)>,
    )>,

    catcher: Receiver<(i32, String, SyncSender<i32>)>, //to_lua_rx: Mutex<Receiver<(String, String, LuaEnt, SyncSender<Option<LuaEnt>>)>>,
}

impl LuaCore {
    pub fn new(switch_board: Arc<RwLock<SwitchBoard>>) -> LuaCore {
        log("lua core thread started".to_string());

        let (rec, catcher) = start(switch_board);
        LuaCore {
            to_lua_tx: rec,
            catcher,
        }
    }

    pub fn start(&mut self, switch_board: Arc<RwLock<SwitchBoard>>) {
        let (rec, catcher) = start(switch_board);
        self.to_lua_tx = rec;
        self.catcher = catcher;
        // reset()
    }

    pub fn call(&self, func: &String, ent: LuaEnt) -> LuaEnt {
        let r = self.inject(func, &"".to_string(), Some(ent.clone()));
        match r.0 {
            Some(message) => {
                println!("lua_call::{}", message);
            }
            _ => {}
        }
        match r.1 {
            Some(ento) => {
                // println!("ye");
                ento
            }
            None => ent,
        }
    }

    pub fn func(&self, func: &String) -> String {
        //
        match self.inject(func, &"0".to_string(), None).0 {
            Some(str) => {
                // println!("attempt {}", str);
                str
            }
            None => "".to_string(),
        }
    }

    fn inject(
        &self,
        func: &String,
        path: &String,
        ent: Option<LuaEnt>,
    ) -> (Option<String>, Option<LuaEnt>) {
        let (tx, rx) = sync_channel::<(Option<String>, Option<LuaEnt>)>(0);
        let bool = ent.is_some();
        // println!("xxx {} :: {}", func, path);
        self.to_lua_tx.send((func.clone(), path.clone(), ent, tx));

        //MutexGuard::unlock_fair(guard);
        match rx.recv() {
            Ok(lua_out) => lua_out,
            Err(e) => {
                err(e.to_string());
                (None, None)
            }
        }
    }

    pub fn load(&self, file: &String) {
        log("loading script".to_string());
        self.inject(&"load".to_string(), file, None);
    }

    pub fn call_main(&self) {
        self.func(&"main()".to_string());
    }

    pub fn call_loop(&self) {
        self.func(&"loop()".to_string());
    }

    pub fn die(&self) {
        log("lua go bye bye".to_string());
        self.inject(&"load".to_string(), &"_self_destruct".to_string(), None);
    }

    pub fn spawn(&self, str: &String) {
        //entity_factory.create_ent(str, self);
    }

    // pub fn load(&self, input_path: &String) {
    //     // let input_path = Path::new(".")
    //     //     .join("scripts")
    //     //     .join(str.to_owned())
    //     //     .with_extension("lua");

    //     let name = crate::asset::get_file_name(input_path.to_owned());
    //     let st = fs::read_to_string(input_path).unwrap_or_default();
    //     log(format!("got script {} :\n{}", input_path, st));
    //     let chunk = self.lua.load(&st);
    //     let globals = self.lua.globals();
    //     //chunk.eval()
    //     //let d= chunk.eval::<mlua::Chunk>();

    //     match chunk.eval::<mlua::Function>() {
    //         Ok(code) => {
    //             log(format!("code loaded {} ♥", name));
    //             globals.set(name, code);
    //         }
    //         Err(err) => {
    //             println!(
    //                 "::lua::  bad lua code for 📜{} !! Assigning default \"{}\"",
    //                 name, err
    //             );
    //             globals.set(name, globals.get::<_, Function>("default_func").unwrap());
    //         }
    //     }
    // }

    //let out = self.lua.globals().get("default_func").unwrap();
    //out

    // pub fn get(&self, str: String) -> Function {
    //     let globals = self.lua.globals();
    //     //let version = globals.get::<_, String>("_VERSION").unwrap();
    //     let res = globals.get::<_, Function>(str.to_owned());
    //     if res.is_err() {
    //         self.load(&str.to_owned());
    //         let res2 = globals.get::<_, Function>(str.to_owned());
    //         if res2.is_err() {
    //             log(format!(
    //                 "failed to get lua code for 📜{} even after default func",
    //                 str
    //             ));
    //         }
    //         log(format!(
    //             "we didnt find lua code so we loaded it and returned it for 📜{}",
    //             str
    //         ));
    //         res2.unwrap()
    //     } else {
    //         log(format!("we got and returned a func for {}", str));
    //         res.unwrap()
    //     }
    // }
}
fn lua_load(lua: &Lua, st: &String) {
    // log(format!("got script :\n{}", st));
    let chunk = lua.load(&st);
    // chunk.eval()
    if let Err(s) = chunk.eval::<()>() {
        println!("exec {}", s);
    }

    //::<mlua::Function>
    // chunk.call(1);
    // match chunk.exec() {
    //     Ok(f) => log(format!("code loaded  ♥")),
    //     Err(err) => log(format!("yucky code {}", err)),
    // }
    // lua.load_from_function(modname, func)
}

// fn lua_load_classic(lua: &Lua, st: &String) {
//     let name = st;
//     // let input_path = Path::new(".")
//     //     .join("scripts")
//     //     .join(str.to_owned())
//     //     .with_extension("lua");
//     // log(format!("script in as {}", input_path));
//     // let name = crate::asset::get_file_name(input_path.to_owned());
//     // let st = fs::read_to_string(input_path).unwrap_or_default();
//     log(format!("got script :\n{}", st));
//     let chunk = lua.load(&st);
//     let globals = lua.globals();
//     //chunk.eval()
//     //let d= chunk.eval::<mlua::Chunk>();

//     match chunk.eval::<mlua::Function>() {
//         Ok(code) => {
//             log(format!("code loaded {} ♥", name));
//             globals.set(name, code);
//         }
//         Err(err) => {
//             println!(
//                 "::lua::  bad lua code for 📜{} !! Assigning default \"{}\"",
//                 name, err
//             );
//             // default needs to exist, otherwise... i don't know? crash the whole lua thread is probably best
//             globals.set(name, globals.get::<_, Function>("_default_func").unwrap());
//         }
//     }
// }

// pub fn scope_test<'b, 'scope>(
//     scope: &Scope<'scope, 'b>,
//     ent_factory: &'b EntFactory,
//     lua_core: &'b LuaCore,
//     meshes: Rc<RefCell<Vec<Ent<'b>>>>,
// ) {
//     let closure = move |_, (str, x, y): (String, f32, f32)| {
//         let mut ent = ent_factory.create_ent(&str, &lua_core);
//         ent.pos.x = x;
//         ent.pos.y = y;
//         let lua_ent = ent.to_lua();
//         let res = meshes.try_borrow_mut();
//         if res.is_ok() {
//             let mut m = res.unwrap();
//             m.push(ent);
//             println!("added ent, now sized at {}", m.len());
//         } else {
//             println!("cannot add ent, overworked!")
//         }
//         Ok(lua_ent)
//         //Ok(&ent.to_lua())
//     };

//     let lua_globals = lua_core.lua.globals();
//     lua_globals.set("spawn", {
//         let m = scope.create_function(closure);
//         m.unwrap()
//     });
// }

// pub fn test() {
//     let mut env = minijinja::Environment::new();

//     let (to_lua_tx, to_lua_rx) = channel::<(String, String, SyncSender<String>)>();

//     thread::spawn(move || {
//         let lua = rlua::Lua::new();
//         lua.context(move |lua_ctx| {
//             lua_ctx.load("some_code").exec().unwrap();
//             let globals = lua_ctx.globals();
//             let temple: rlua::Table = globals.get("temple").unwrap();
//             let filters: rlua::Table = temple.get("_filters").unwrap();
//             let concat2: rlua::Function = filters.get("concat2").unwrap();
//             while let Ok((s1, s2, channel)) = to_lua_rx.recv() {
//                 let res: String = concat2.call::<_, String>((s1, s2)).unwrap();
//                 channel.send(res).unwrap()
//             }
//         })
//     });

//     let to_lua_tx = Mutex::new(to_lua_tx);
//     env.add_filter(
//         "concat2",
//         move |_env: &minijinja::Environment,
//               s1: String,
//               s2: String|
//               -> anyhow::Result<String, minijinja::Error> {
//             let (tx, rx) = sync_channel::<String>(0);
//             to_lua_tx.lock().unwrap().send((s1, s2, tx)).unwrap();
//             let res = rx.recv().unwrap();
//             Ok(res)
//         },
//     );
// }

fn start(
    switch_board: Arc<RwLock<SwitchBoard>>,
) -> (
    Sender<(
        String,
        String,
        Option<LuaEnt>,
        SyncSender<(Option<String>, Option<LuaEnt>)>,
    )>,
    Receiver<(i32, String, SyncSender<i32>)>,
) {
    let (sender, reciever) = channel::<(
        String,
        String,
        Option<LuaEnt>,
        SyncSender<(Option<String>, Option<LuaEnt>)>,
    )>();

    let (pitcher, catcher) = channel::<(i32, String, SyncSender<i32>)>();

    log("init lua core".to_string());
    let lua_thread = thread::spawn(move || {
        let lua_ctx = Lua::new();

        let globals = lua_ctx.globals();

        crate::command::init_lua_sys(&lua_ctx, &globals, switch_board, &pitcher);

        log("💫 lua_thread::orbiting".to_string());
        for m in reciever {
            //}
            let (s1, s2, ent, channel) = m;

            //while let Ok((s1, s2, ent, channel)) = reciever.recv() {
            //println!("➡️ lua_thread::recieved");

            // println!("we have a func! {:?}", res.clone().unwrap());
            if ent.is_some() {
                let res = globals.get::<_, Function>(s1.to_owned());
                if res.is_ok() {
                    match res.unwrap().call::<LuaEnt, LuaEnt>(ent.unwrap()) {
                        Ok(o) => channel.send((None, Some(o))).unwrap(),
                        Err(er) => channel.send((None, None)).unwrap(),
                    }
                } else {
                    channel.send((None, None));
                }
            } else {
                if s1 == "load" {
                    if s2 == "_self_destruct" {
                        break;
                    } else {
                        lua_load(&lua_ctx, &s2);
                    }
                } else {
                    // TODO load's chunk should call set_name to "main" etc, for better error handling
                    match match lua_ctx.load(&s1).eval::<String>() {
                        //res.unwrap().call::<String, String>(s2.to_owned()) {
                        Ok(o) => channel.send((Some(o), None)),
                        Err(er) => channel.send((Some(er.to_string()), None)),
                    } {
                        Ok(s) => {}
                        Err(e) => log(format!("lua server communication error occured -> {}", e)),
                    }
                    // }
                }
            }

            /*  TODO is this still needed?
            match globals.get::<&str, mlua::Table>("_ents") {
                Ok(table) => {
                    let ent_results = table.sequence_values::<LuaEnt>();

                    let mut ent_array = ent_results.filter_map(|g| g.ok()).collect::<Vec<_>>();

                    match ent_master.try_write_for(std::time::Duration::from_millis(100)) {
                        Some(mut ent_guard) => {
                            match ent_guard.get_mut() {
                                Some(entman) => {
                                    println!("ent_man adjusted {}", ent_array.len());
                                    entman.ent_table.clear();
                                    entman.ent_table.append(&mut ent_array);
                                }
                                None => {}
                            }
                            drop(ent_guard);
                        }
                        _ => {}
                    }
                }
                Err(er) => {
                    log("missing highest level entities table".to_string());
                }
            }
            */

            //thread::sleep(std::time::Duration::from_millis(10));
            //let res: String = concat2.call::<_, String>((s1, s2)).unwrap();
            //channel.send(res).unwrap()
        }

        //})
    });
    (sender, catcher)
}

fn log(str: String) {
    println!("📜lua::{}", str);
    crate::log::log(format!("📜lua::{}", str));
}

fn err(str: String) {
    println!("📜lua_err::{}", str);
    crate::log::log(format!("📜lua_err::{}", str));
}

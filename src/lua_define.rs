use crate::{ent::Ent, lua_ent::LuaEnt};
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
        mpsc::{channel, sync_channel, Receiver, Sender, SyncSender},
        Arc,
    },
    thread,
};

pub struct LuaCore {
    // pub lua: Mutex<mlua::Lua>,
    to_lua_tx: Mutex<Sender<(String, String, Option<LuaEnt>, SyncSender<Option<LuaEnt>>)>>,
    //to_lua_rx: Mutex<Receiver<(String, String, LuaEnt, SyncSender<Option<LuaEnt>>)>>,
}

impl LuaCore {
    pub fn new() -> LuaCore {
        let (sender, reciever) =
            channel::<(String, String, Option<LuaEnt>, SyncSender<Option<LuaEnt>>)>();

        let lua_thread = thread::spawn(move || {
            let lua_ctx = Lua::new();

            // lua.context(move |lua_ctx| {
            //lua_ctx.load("some_code").exec().unwrap();
            // let lua_clone = Arc::clone(&crate::lua_master);
            let globals = lua_ctx.globals();

            let multi = lua_ctx.create_function(|_, (x, y): (f32, f32)| Ok(x * y));
            globals.set("multi", multi.unwrap());

            let closure = |_, (str, x, y): (String, f32, f32)| {
                //let result = entity_factory.lock().unwrap().get_mut();
                // if result.is_some() {
                //     // let ent = result.unwrap().create_ent(&str, &self);
                //     // ent.pos.x = x;
                //     // ent.pos.y = y;
                //     // let lua_ent = ent.to_lua();
                //     // // let mut m = meshes.borrow_mut();
                //     // // m.push(ent);
                //     // // println!("added ent, now sized at {}", m.len());
                //     // Ok(lua_ent)
                //     Ok(LuaEnt::empty())
                // } else {
                //     Ok(LuaEnt::empty())
                // }

                //     //Ok(&ent.to_lua())
                Ok(LuaEnt::empty())
            };
            globals.set("spawn", {
                let m = lua_ctx.create_function(closure);
                m.unwrap()
            });
            let default_func = lua_ctx.create_function(|_, e: f32| Ok(e)).unwrap();
            globals.set("default_func", default_func);

            // let temple: rlua::Table = globals.get("temple").unwrap();
            // let filters: rlua::Table = temple.get("_filters").unwrap();
            // let concat2: rlua::Function = filters.get("concat2").unwrap();
            println!("💫 lua_thread::orbiting");
            for m in reciever {
                //}
                let (s1, s2, ent, channel) = m;

                //while let Ok((s1, s2, ent, channel)) = reciever.recv() {
                //println!("➡️ lua_thread::recieved");
                if s1 == "load" {
                    lua_load(&lua_ctx, &s2);
                } else {
                    let res = globals.get::<_, Function>(s1.to_owned());

                    if res.is_ok() && ent.is_some() {
                        match res.unwrap().call::<LuaEnt, LuaEnt>(ent.unwrap()) {
                            Ok(o) => channel.send(Some(o)).unwrap(),
                            Err(er) => channel.send(None).unwrap(),
                        }
                    } else {
                        channel.send(None).unwrap()
                    }
                }
                //thread::sleep(std::time::Duration::from_millis(10));
                //let res: String = concat2.call::<_, String>((s1, s2)).unwrap();
                //channel.send(res).unwrap()
            }
            //})
        });
        //lua_thread.join();
        log("lua core thread started".to_string());

        LuaCore {
            // lua:Mutex::new(lua)
            to_lua_tx: Mutex::new(sender),
            // to_lua_rx: Mutex::new(to_lua_rx),
        }
    }

    pub fn init(
        &self,
        // entity_factory: Arc<Mutex<OnceCell<EntFactory>>>,
        // meshes: Rc<RefCell<Vec<Ent<'a>>>>,
    ) {

        //let to_lua_tx = Mutex::new(to_lua_tx);
        // env.add_filter(
        //     "concat2",
        //     move |_env: &minijinja::Environment,
        //           s1: String,
        //           s2: String|
        //           -> anyhow::Result<String, minijinja::Error> {
        //         let (tx, rx) = sync_channel::<String>(0);
        //         to_lua_tx.lock().unwrap().send((s1, s2, tx)).unwrap();
        //         let res = rx.recv().unwrap();
        //         Ok(res)
        //     },
        // );

        // let globals = self.lua.globals();

        //let multi = self.lua.create_function(|_, (x, y): (f32, f32)| Ok(x * y));
        // let t = multi.unwrap();
        //globals.set("multi", multi.unwrap());

        //drop(globals);
    }

    pub fn call(&self, func: String, ent: LuaEnt) -> LuaEnt {
        match self.inject(func, "".to_string(), Some(ent.clone())) {
            Some(ento) => {
                //println!("returned");
                ento
            }
            None => ent,
        }
    }
    fn inject(&self, func: String, path: String, ent: Option<LuaEnt>) -> Option<LuaEnt> {
        let (tx, rx) = sync_channel::<Option<LuaEnt>>(0);
        let guard = self.to_lua_tx.lock();
        let bool = ent.is_some();
        guard.send((func, path, ent, tx));
        MutexGuard::unlock_fair(guard);
        if bool {
            match rx.recv() {
                Ok(lua_out) => {
                    // println!(
                    //     "returned {}",
                    //     match &lua_out {
                    //         Some(l) => l.x,
                    //         None => -1.,
                    //     }
                    // );
                    lua_out
                }
                Err(e) => None,
            }
        } else {
            None
        }

        // let (tx, rx) = sync_channel::<String>(0);
        //             to_lua_tx.lock().unwrap().send((s1, s2, tx)).unwrap();
        //             let res = rx.recv().unwrap();

        //self.to_lua_tx.
    }
    pub fn load(&self, file: String) {
        log("loading script".to_string());
        self.inject("load".to_string(), file, None);
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

fn lua_load(lua: &Lua, input_path: &String) {
    // let input_path = Path::new(".")
    //     .join("scripts")
    //     .join(str.to_owned())
    //     .with_extension("lua");
    log(format!("script in as {}", input_path));
    let name = crate::asset::get_file_file_name(input_path.to_owned());
    let st = fs::read_to_string(input_path).unwrap_or_default();
    log(format!("got script {} :\n{}", input_path, st));
    let chunk = lua.load(&st);
    let globals = lua.globals();
    //chunk.eval()
    //let d= chunk.eval::<mlua::Chunk>();

    match chunk.eval::<mlua::Function>() {
        Ok(code) => {
            log(format!("code loaded {} ♥", name));
            globals.set(name, code);
        }
        Err(err) => {
            println!(
                "::lua::  bad lua code for 📜{} !! Assigning default \"{}\"",
                name, err
            );
            globals.set(name, globals.get::<_, Function>("default_func").unwrap());
        }
    }
}

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

fn log(str: String) {
    crate::log::log(format!("📜lua::{}", str));
}

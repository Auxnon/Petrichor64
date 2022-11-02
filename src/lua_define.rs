use crate::{
    command::MainCommmand,
    controls::ControlState,
    pad::Pad,
    sound::SoundPacket,
    switch_board::SwitchBoard,
    world::{TileCommand, TileResponse},
};
use gilrs::{Axis, Button, Event, EventType, Gilrs};
use mlua::Lua;
use parking_lot::{Mutex, RwLock};
use std::{
    rc::Rc,
    sync::{
        mpsc::{channel, sync_channel, Receiver, Sender, SyncSender},
        Arc,
    },
    thread,
};

pub type MainPacket = MainCommmand;

pub struct LuaCore {
    // pub lua: Mutex<mlua::Lua>,
    to_lua_tx: Sender<(
        String,
        String,
        Option<ControlState>,
        Option<SyncSender<(Option<String>, Option<ControlState>)>>,
    )>,
}

impl LuaCore {
    pub fn new(
        switch_board: Arc<RwLock<SwitchBoard>>,
        world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        singer: Sender<SoundPacket>,
        dangerous: bool,
    ) -> LuaCore {
        log("lua core thread started".to_string());

        let (rec, _catcher) = start(switch_board, world_sender, singer, dangerous);
        LuaCore {
            to_lua_tx: rec,
            // catcher,
        }
    }

    pub fn start(
        &mut self,
        switch_board: Arc<RwLock<SwitchBoard>>,
        world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        singer: Sender<SoundPacket>,
        dangerous: bool,
    ) -> Receiver<MainPacket> {
        let (rec, catcher) = start(switch_board, world_sender, singer, dangerous);
        self.to_lua_tx = rec;
        // self.catcher = catcher;
        // reset()
        catcher
    }

    pub fn func(&self, func: &String) -> String {
        match self.inject(func, &"0".to_string(), None).0 {
            Some(str) => str,
            None => "".to_string(),
        }
    }

    pub fn async_func(&self, func: &String, bits: ControlState) {
        self.async_inject(func, Some(bits));
    }

    fn inject(
        &self,
        func: &String,
        path: &String,
        ent: Option<ControlState>,
    ) -> (Option<String>, Option<ControlState>) {
        let (tx, rx) = sync_channel::<(Option<String>, Option<ControlState>)>(0);
        // println!("xxx {} :: {}", func, path);
        match self
            .to_lua_tx
            .send((func.clone(), path.clone(), ent, Some(tx)))
        {
            Ok(_) => match rx.recv() {
                Ok(lua_out) => lua_out,
                Err(e) => {
                    log(format!("unable to recieve lua return command {}", e));
                    err(e.to_string());
                    (None, None)
                }
            },
            Err(e) => {
                err(format!("unable to inject lua command {}", e));
                (None, None)
            }
        }
    }

    fn async_inject(&self, func: &String, bits: Option<ControlState>) {
        // let (tx, rx) = channel::<(Option<String>, Option<ControlState>)>();
        match self
            .to_lua_tx
            .send((func.clone(), "".to_string(), bits, None))
        {
            Ok(_) => {}
            _ => {}
        }
    }

    pub fn load(&self, file: &String) {
        log("loading script".to_string());
        self.inject(&"load".to_string(), file, None);
    }

    pub fn call_main(&self) {
        const empty: ControlState = ([false; 256], [0f32; 4]);
        self.async_func(&"main()".to_string(), empty);

        log("called main method of main script".to_string());
    }

    pub fn call_loop(&self, bits: ControlState) {
        self.async_func(&"loop()".to_string(), bits);
    }

    pub fn die(&self) {
        log("lua go bye bye".to_string());
        self.async_inject(&"_self_destruct".to_string(), None);
        // self.inject(&"load".to_string(), &"_self_destruct".to_string(), None);
    }
}
fn lua_load(lua: &Lua, st: &String) {
    let chunk = lua.load(st);

    chunk.exec();
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
    world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
    singer: Sender<SoundPacket>,
    dangerous: bool,
) -> (
    Sender<(
        String,
        String,
        Option<ControlState>,
        Option<SyncSender<(Option<String>, Option<ControlState>)>>,
    )>,
    Receiver<MainPacket>,
) {
    let (sender, reciever) = channel::<(
        String,
        String,
        Option<ControlState>,
        Option<SyncSender<(Option<String>, Option<ControlState>)>>,
    )>();

    let (pitcher, catcher) = channel::<MainPacket>();

    let mut online = false;
    #[cfg(feature = "online_capable")]
    let mut closer = None;
    #[cfg(not(feature = "online_capable"))]
    let mut closer: Option<bool> = None;
    #[cfg(feature = "online_capable")]
    let net = if false {
        match crate::online::init() {
            Ok((nout, nin)) => {
                match nout.send(vec![0f32; 3]) {
                    Ok(s) => {
                        closer = Some(nout.clone());
                        online = true;
                    }
                    Err(e) => println!("pre send failed at {}", e),
                }
                Some((nout, nin))
            }
            _ => None,
        }
    } else {
        None
    };

    #[cfg(not(feature = "online_capable"))]
    let net: Option<bool> = None;

    // let pitchers = Arc::new(pitcher);
    // let pitch_lusa = Arc::clone(&pitchers);

    log("init lua core".to_string());
    // let lua_thread =
    thread::spawn(move || {
        let keys = [false; 256];
        let mice = [0.; 4];

        let keys_mutex = Rc::new(Mutex::new(keys));
        let mice_mutex = Rc::new(Mutex::new(mice));
        let ent_counter = Rc::new(Mutex::new(2u64));

        let lua_ctx = if true {
            unsafe { Lua::unsafe_new_with(mlua::StdLib::ALL, mlua::LuaOptions::new()) }
        } else {
            Lua::new()
        };
        // lua_ctx.load_from_std_lib(mlua::StdLib::DEBUG);
        // lua_ctx.sa

        let globals = lua_ctx.globals();

        log("new controller connector starting".to_string());
        let mut gilrs = Gilrs::new().unwrap();
        for (_id, gamepad) in gilrs.gamepads() {
            log(format!("{} is {:?}", gamepad.name(), gamepad.power_info()));
        }

        let pads = Rc::new(Mutex::new(Pad::new()));

        let error_sender = pitcher.clone();
        let mut debounce_error_string = "".to_string();
        let mut debounce_error_counter = 60;
        match crate::command::init_lua_sys(
            &lua_ctx,
            &globals,
            // switch_board,
            pitcher,
            world_sender,
            net,
            singer,
            Rc::clone(&keys_mutex),
            Rc::clone(&mice_mutex),
            Rc::clone(&pads),
            Rc::clone(&ent_counter),
        ) {
            Err(err) => {
                log(format!("lua command injection failed: {}", err));
            }
            _ => {
                log("lua commands initialized".to_string());
            }
        }

        log("💫 lua_thread::orbiting".to_string());
        for m in reciever {
            let (s1, s2, bit_in, channel) = m;
            while let Some(Event {
                id: _,
                event,
                time: _,
            }) = gilrs.next_event()
            {
                // println!("{:?} New event from {}: {:?}", time, id, event);
                match event {
                    EventType::ButtonPressed(button, _) => {
                        match button {
                            Button::Start => pads.lock().start = 1.0,
                            Button::South => pads.lock().south = 1.0,
                            Button::East => pads.lock().east = 1.0,
                            Button::West => pads.lock().west = 1.0,
                            Button::North => pads.lock().north = 1.0,

                            // Button::Z => pads.lock().z = 1.0,
                            // Button::C => pads.lock().c = 1.0,
                            Button::DPadUp => pads.lock().dup = 1.0,
                            Button::DPadDown => pads.lock().ddown = 1.0,
                            Button::DPadLeft => pads.lock().dleft = 1.0,
                            Button::DPadRight => pads.lock().dright = 1.0,
                            _ => {}
                        }
                    }
                    EventType::ButtonReleased(button, _) => match button {
                        Button::Start => pads.lock().start = 0.,
                        Button::South => pads.lock().south = 0.,
                        Button::East => pads.lock().east = 0.,
                        Button::West => pads.lock().west = 0.,
                        Button::North => pads.lock().north = 0.,
                        // Button::Z => pads.lock().z = 0.,
                        // Button::C => pads.lock().c = 0.,
                        Button::DPadUp => pads.lock().dup = 0.,
                        Button::DPadDown => pads.lock().ddown = 0.,
                        Button::DPadLeft => pads.lock().dleft = 0.,
                        Button::DPadRight => pads.lock().dright = 0.,

                        _ => {}
                    },
                    EventType::AxisChanged(axis, value, _) => match axis {
                        Axis::LeftStickX => pads.lock().laxisx = value,
                        Axis::LeftStickY => pads.lock().laxisy = value,
                        //         Axis::LeftZ => todo!(),
                                Axis::RightStickX => pads.lock().raxisx = value,
                                Axis::RightStickY => pads.lock().raxisy = value,
                        //         Axis::RightZ => todo!(),
                        //         Axis::DPadX => todo!(),
                        //         Axis::DPadY => todo!(),
                        _ => {}
                    },
                    _ => {}
                    //     EventType::ButtonRepeated(_, _) => todo!(),
                    //     EventType::ButtonChanged(_, _, _) => todo!(),
                    //     EventType::Connected => todo!(),
                    //     EventType::Disconnected => todo!(),
                    //     EventType::Dropped => todo!(),
                }
            }
            // let pd = pads.lock();
            // println!("buttons {} {}", pd.laxisx, pd.laxisy);
            // drop(pd);

            if s1 == "load" {
                // println!("load 1{}", s2);
                if s2 == "_self_destruct" {
                    // println!("closing 1");
                    #[cfg(feature = "online_capable")]
                    match closer {
                        Some(n) => {
                            // println!("closing 2");
                            n.send(vec![-99., 0., 0.]);
                        }
                        _ => {}
                    }
                    break;
                } else {
                    lua_load(&lua_ctx, &s2);
                }
            } else {
                //MARK if loop() then path string is the keyboard keys

                if s1 == "_self_destruct" {
                    #[cfg(feature = "online_capable")]
                    match closer {
                        Some(n) => {
                            n.send(vec![-99., 0., 0.]);
                        }
                        _ => {}
                    }
                    break;
                }

                // TODO load's chunk should call set_name to "main" etc, for better error handling
                let res = lua_ctx.load(&s1).eval::<mlua::Value>();
                match channel {
                    Some(sync) => {
                        match match res {
                            Ok(o) => {
                                let output = format!("{:?}", o);
                                sync.send((Some(output), None))
                            }
                            Err(er) => sync.send((Some(er.to_string()), None)),
                        } {
                            Err(e) => {
                                // println!("loop err: {}", e);
                                // if !s1.starts_with("loop") {
                                err(format!("lua server communication error occured -> {}", e))
                                // }
                            }
                            _ => {}
                        };
                    }
                    _ => {
                        //=== async functions error handler will debounce since we deal with rapid event looping ===
                        match res {
                            Err(e) => {
                                debounce_error_string = format!("{}", e);
                                debounce_error_counter += 1;
                                if debounce_error_counter >= 60 {
                                    debounce_error_counter = 0;
                                    match lua_ctx.inspect_stack(1) {
                                        Some(d) => {
                                            // d.names().name
                                            println!("stack {:?}", d.stack());
                                            println!("er line is {}", d.curr_line());
                                        }
                                        _ => {}
                                    };
                                    error_sender.send(crate::MainCommmand::AsyncError(
                                        debounce_error_string,
                                    ));
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // {
                //     Err(e) => {
                //         if !s1.starts_with("loop") {
                //             err(format!("lua server communication error occured -> {}", e))
                //         }
                //     }
                //     _ => {}
                // }
                match bit_in {
                    Some(b) => {
                        *keys_mutex.lock() = b.0;
                        *mice_mutex.lock() = b.1;
                    }
                    _ => {}
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

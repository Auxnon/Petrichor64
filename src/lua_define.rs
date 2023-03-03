#[cfg(feature = "audio")]
use crate::sound::SoundCommand;
use crate::{
    bundle::BundleResources,
    command::MainCommmand,
    controls::ControlState,
    log::LogType,
    pad::Pad,
    world::{TileCommand, TileResponse},
};
use gilrs::{Axis, Button, Event, EventType, Gilrs};
use mlua::{
    prelude::LuaError,
    Lua,
    Value::{self, Nil},
};
use parking_lot::Mutex;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::mpsc::{channel, sync_channel, Sender, SyncSender},
    thread,
    time::Duration,
};

#[cfg(feature = "audio")]
pub type SoundSender = Sender<SoundCommand>;
#[cfg(not(feature = "audio"))]
pub type SoundSender = ();

pub type MainPacket = (u8, MainCommmand);

pub type LuaHandle = thread::JoinHandle<()>;

pub enum LuaResponse {
    String(String),
    Number(f64),
    Integer(i32),
    Boolean(bool),
    Table(HashMap<String, String>),
    TableOfTuple(HashMap<String, (String, String)>),
    Nil,
    Error(String),
}

pub enum LuaTalk {
    AsyncFunc(String),
    Func(String, SyncSender<LuaResponse>),
    Main,
    Loop(ControlState),
    Load(String, SyncSender<LuaResponse>),
    AsyncLoad(String),
    Resize(u32, u32),
    Die,
    Drop(String),
}

pub struct LuaCore {
    to_lua_tx: Sender<LuaTalk>,
}

impl LuaCore {
    /** create new but do not start yet. Channel acts as a placeholder */
    pub fn new(// bundle_id: u8,
        // gui: GuiMorsel,
        // world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        // singer: Sender<SoundPacket>,
        // dangerous: bool,
    ) -> LuaCore {
        let (sender, reciever) = channel::<LuaTalk>();

        LuaCore { to_lua_tx: sender }
    }

    pub fn start(
        &mut self,
        bundle_id: u8,
        resources: BundleResources,
        world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        pitcher: Sender<MainPacket>,
        loggy: Sender<(LogType, String)>,
        singer: SoundSender,
        debug: bool,
        dangerous: bool,
    ) -> LuaHandle {
        let (rec, lua_handle) = start(
            bundle_id,
            resources,
            world_sender,
            pitcher,
            loggy,
            singer,
            debug,
            dangerous,
        );
        self.to_lua_tx = rec;
        lua_handle
    }

    pub fn func(&self, func: &str) -> LuaResponse {
        let (tx, rx) = sync_channel::<LuaResponse>(0);
        // self.inject(func, &"0", None).0
        self.to_lua_tx.send(LuaTalk::Func(func.to_string(), tx));
        match rx.recv_timeout(Duration::from_millis(4000)) {
            Ok(lua_out) => lua_out,
            Err(e) => LuaResponse::Error(format!("No/slow response from lua -> {}", e)),
        }
    }

    // pub fn async_func(&self, func: &String, bits: ControlState) {
    //     self.async_inject(func, Some(bits));
    // }

    // fn inject(
    //     &self,
    //     func: &str,
    //     path: &str,
    //     ent: Option<ControlState>,
    // ) -> (LuaResponse, Option<ControlState>) {
    //     let (tx, rx) = sync_channel::<(LuaResponse, Option<ControlState>)>(0);
    //     // println!("xxx {} :: {}", func, path);
    //     match self
    //         .to_lua_tx
    //         .send((func.to_string(), path.to_string(), ent, Some(tx)))
    //     {
    //         Ok(_) => match rx.recv() {
    //             Ok(lua_out) => lua_out,
    //             Err(e) => (
    //                 LuaResponse::Error(format!("No response from lua: {}", e)),
    //                 None,
    //             ),
    //         },
    //         Err(e) => (
    //             LuaResponse::Error(format!("Cannot speak to lua: {}", e)),
    //             None,
    //         ),
    //     }
    // }

    // fn async_inject(&self, func: &String, bits: Option<ControlState>) {
    //     match self
    //         .to_lua_tx
    //         .send((func.clone(), "".to_string(), bits, None))
    //     {
    //         Ok(_) => {}
    //         _ => {}
    //     }
    // }

    pub fn load(&self, file: &String) -> LuaResponse {
        // log("loading script".to_string());
        // self.inject(&"load".to_string(), file, None)

        let (tx, rx) = sync_channel::<LuaResponse>(0);
        match self.to_lua_tx.send(LuaTalk::Load(file.to_string(), tx)) {
            Ok(_) => match rx.recv_timeout(Duration::from_millis(10000)) {
                Ok(lua_out) => lua_out,
                Err(e) => LuaResponse::Error(format!("No / >10s response from lua -> {}", e)),
            },
            Err(e) => LuaResponse::Error(format!("Cannot speak to lua: {}", e)),
        }
    }

    pub fn resize(&self, w: u32, h: u32) {
        self.to_lua_tx.send(LuaTalk::Resize(w, h));
    }

    pub fn async_load(&self, file: &String) {
        self.to_lua_tx.send(LuaTalk::AsyncLoad(file.to_string()));
    }

    pub fn call_main(&self) {
        self.to_lua_tx.send(LuaTalk::Main);
    }
    pub fn call_drop(&self, s: String) {
        self.to_lua_tx.send(LuaTalk::Drop(s));
    }

    pub fn call_loop(&self, bits: ControlState) {
        self.to_lua_tx.send(LuaTalk::Loop(bits));
    }

    /** sends kill signal to this lua context thread */
    pub fn die(&self) {
        // self.async_inject(&"_self_destruct".to_string(), None);
        self.to_lua_tx.send(LuaTalk::Die);
    }
}
fn lua_load(lua: &Lua, st: &String) -> Result<(), LuaError> {
    let chunk = lua.load(st);

    chunk.exec()
}

fn start(
    // switch_board: Arc<RwLock<SwitchBoard>>,
    bundle_id: u8,
    resources: BundleResources,
    world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
    pitcher: Sender<MainPacket>,
    loggy: Sender<(LogType, String)>,
    singer: SoundSender,
    debug: bool,
    dangerous: bool,
) -> (Sender<LuaTalk>, LuaHandle) {
    let (sender, reciever) = channel::<
        LuaTalk, // String,
                 // String,
                 // Option<ControlState>,
                 // Option<SyncSender<(LuaResponse, Option<ControlState>)>>,
    >();

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

    loggy.send((LogType::LuaSys, format!("init lua core {}", bundle_id)));
    // let lua_thread =
    let thread_join = thread::spawn(move || {
        let keys = [false; 256];
        let mice = [0.; 13];

        let keys_mutex = Rc::new(RefCell::new(keys));
        let diff_keys_mutex = Rc::new(RefCell::new([false; 256]));
        let mice_mutex = Rc::new(RefCell::new(mice));
        let ent_counter = Rc::new(Mutex::new(2u64));

        let gui_handle = Rc::new(RefCell::new(crate::gui::GuiMorsel::new(resources)));

        let lua_ctx = if true {
            unsafe { Lua::unsafe_new_with(mlua::StdLib::ALL, mlua::LuaOptions::new()) }
        } else {
            Lua::new()
        };
        // lua_ctx.load_from_std_lib(mlua::StdLib::DEBUG);
        // lua_ctx.sa

        let globals = lua_ctx.globals();

        if debug {
            loggy.send((
                LogType::LuaSys,
                "new controller connector starting".to_owned(),
            ));
        }
        let mut gilrs = Gilrs::new().unwrap();
        for (_id, gamepad) in gilrs.gamepads() {
            loggy.send((
                LogType::LuaSys,
                format!("gamepad {} is {:?}", gamepad.name(), gamepad.power_info()),
            ));
        }

        let pads = Rc::new(RefCell::new(Pad::new()));

        let async_sender = pitcher.clone();
        let mut debounce_error_string = "".to_string();
        let mut debounce_error_counter = 60;
        match crate::command::init_lua_sys(
            &lua_ctx,
            &globals,
            bundle_id,
            pitcher,
            world_sender,
            Rc::clone(&gui_handle),
            net,
            singer,
            Rc::clone(&keys_mutex),
            Rc::clone(&diff_keys_mutex),
            Rc::clone(&mice_mutex),
            Rc::clone(&pads),
            Rc::clone(&ent_counter),
            loggy.clone(),
        ) {
            Err(err) => {
                loggy.send((
                    LogType::LuaSysError,
                    format!("lua command injection failed: {}", err),
                ));
            }
            _ => {
                if debug {
                    loggy.send((LogType::LuaSys, "lua commands initialized".to_owned()));
                }
            }
        }
        if debug {
            loggy.send((LogType::LuaSys, "begin lua system listener".to_owned()));
        }
        for m in reciever {
            // let (s1, s2, bit_in, channel) = m;
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
                            Button::Start => pads.borrow_mut().start = 1.0,
                            Button::South => pads.borrow_mut().south = 1.0,
                            Button::East => pads.borrow_mut().east = 1.0,
                            Button::West => pads.borrow_mut().west = 1.0,
                            Button::North => pads.borrow_mut().north = 1.0,

                            // Button::Z => pads.borrow_mut().z = 1.0,
                            // Button::C => pads.borrow_mut().c = 1.0,
                            Button::DPadUp => pads.borrow_mut().dup = 1.0,
                            Button::DPadDown => pads.borrow_mut().ddown = 1.0,
                            Button::DPadLeft => pads.borrow_mut().dleft = 1.0,
                            Button::DPadRight => pads.borrow_mut().dright = 1.0,
                            _ => {}
                        }
                    }
                    EventType::ButtonReleased(button, _) => match button {
                        Button::Start => pads.borrow_mut().start = 0.,
                        Button::South => pads.borrow_mut().south = 0.,
                        Button::East => pads.borrow_mut().east = 0.,
                        Button::West => pads.borrow_mut().west = 0.,
                        Button::North => pads.borrow_mut().north = 0.,
                        // Button::Z => pads.borrow_mut().z = 0.,
                        // Button::C => pads.borrow_mut().c = 0.,
                        Button::DPadUp => pads.borrow_mut().dup = 0.,
                        Button::DPadDown => pads.borrow_mut().ddown = 0.,
                        Button::DPadLeft => pads.borrow_mut().dleft = 0.,
                        Button::DPadRight => pads.borrow_mut().dright = 0.,

                        _ => {}
                    },
                    EventType::AxisChanged(axis, value, _) => match axis {
                        Axis::LeftStickX => pads.borrow_mut().laxisx = value,
                        Axis::LeftStickY => pads.borrow_mut().laxisy = value,
                        //         Axis::LeftZ => todo!(),
                                Axis::RightStickX => pads.borrow_mut().raxisx = value,
                                Axis::RightStickY => pads.borrow_mut().raxisy = value,
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

            match m {
                LuaTalk::Load(file, sync) => {
                    if let Err(er) = lua_load(&lua_ctx, &file) {
                        loggy.send((LogType::LuaError, er.to_string()));
                        sync.send(LuaResponse::String(er.to_string()));
                    } else {
                        sync.send(LuaResponse::Nil);
                    }
                }
                LuaTalk::AsyncLoad(file) => {
                    if let Err(er) = lua_load(&lua_ctx, &file) {
                        loggy.send((LogType::LuaError, er.to_string()));
                    }
                }
                LuaTalk::Main => {
                    let res = lua_ctx
                        .load(&"main() loop()".to_string())
                        .eval::<mlua::Value>();
                    if let Err(e) = res {
                        if let Some(d) = lua_ctx.inspect_stack(1) {
                            println!("stack {:?}", d.stack());
                            println!("er line is {}", d.curr_line());
                        };
                        async_sender
                            .send((bundle_id, crate::MainCommmand::AsyncError(format!("{}", e))));
                    }
                    // TODO if we just clump a loop in we shouldnt need this right?
                    // async_sender.send((bundle_id, crate::MainCommmand::MainComplete()));
                }
                LuaTalk::Die => {
                    #[cfg(feature = "online_capable")]
                    match closer {
                        Some(n) => {
                            // println!("closing 2");
                            n.send(vec![-99., 0., 0.]);
                        }
                        _ => {}
                    }
                    break;
                }
                LuaTalk::AsyncFunc(func) => {}
                LuaTalk::Loop((key_state, mouse_state)) => {
                    let res = lua_ctx.load(&"loop()".to_string()).eval::<mlua::Value>();
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
                                async_sender.send((
                                    bundle_id,
                                    crate::MainCommmand::AsyncError(debounce_error_string),
                                ));
                            }
                        }
                        _ => {}
                    }

                    // updated with our input information, as this is only provided within the game loop, also send out a gui update

                    let mut h = diff_keys_mutex.borrow_mut();

                    keys_mutex.borrow().iter().enumerate().for_each(|(i, k)| {
                        h[i] = !k && key_state[i];
                    });
                    drop(h);

                    *keys_mutex.borrow_mut() = key_state;
                    // we COULD just copy it but we want to move our current x,y to px,py to track movement deltas
                    let mut mm = mice_mutex.borrow_mut();
                    *mm = [
                        mouse_state[0],
                        mouse_state[1],
                        mouse_state[2],
                        mouse_state[3],
                        mm[4],
                        mm[5],
                        mouse_state[4],
                        mouse_state[5],
                        mouse_state[6],
                        mouse_state[7],
                        mouse_state[8],
                        mouse_state[9],
                        mouse_state[10],
                    ];
                    drop(mm);
                    // mouse_state;

                    // only send if a change was made, otherwise the old image is cached on the main thread

                    async_sender.send((
                        bundle_id,
                        crate::MainCommmand::LoopComplete(gui_handle.borrow_mut().send_state()),
                    ));
                }
                LuaTalk::Func(func, sync) => {
                    // TODO load's chunk should call set_name to "main" etc, for better error handling

                    let res = lua_ctx.load(&func).eval::<mlua::Value>();
                    match match res {
                        Ok(o) => {
                            // let output = format!("{:?}", o);
                            let output = match o {
                                mlua::Value::String(str) => {
                                    let s = str.to_str().unwrap_or(&"").to_string();
                                    LuaResponse::String(s)
                                }
                                mlua::Value::Integer(i) => LuaResponse::Integer(i),
                                mlua::Value::Number(n) => LuaResponse::Number(n),
                                mlua::Value::Boolean(b) => LuaResponse::Boolean(b),
                                mlua::Value::Table(t) => {
                                    let mut hash: HashMap<String, String> = HashMap::new();
                                    let mut hash2: HashMap<String, (String, String)> =
                                        HashMap::new();
                                    for (i, pair) in
                                        t.pairs::<mlua::Value, mlua::Value>().enumerate()
                                    {
                                        if let Ok((k, v)) = pair {
                                            if let mlua::Value::String(key) = k {
                                                match v {
                                                    mlua::Value::String(val) => {
                                                        hash.insert(
                                                            format!(
                                                                "{}",
                                                                key.to_str()
                                                                    .unwrap_or(&i.to_string())
                                                            ),
                                                            format!(
                                                                "{}",
                                                                val.to_str().unwrap_or("")
                                                            ),
                                                        );
                                                    }
                                                    mlua::Value::Table(tt) => {
                                                        if tt.raw_len() == 2 {
                                                            hash2.insert(
                                                                format!(
                                                                    "{}",
                                                                    key.to_str()
                                                                        .unwrap_or(&i.to_string())
                                                                ),
                                                                (
                                                                    tt.get::<_, String>(1)
                                                                        .unwrap_or("".to_owned()),
                                                                    tt.get::<_, String>(2)
                                                                        .unwrap_or("".to_owned()),
                                                                ),
                                                            );
                                                        }
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                    if hash2.len() > 0 {
                                        LuaResponse::TableOfTuple(hash2)
                                    } else {
                                        LuaResponse::Table(hash)
                                    }
                                }
                                mlua::Value::Function(_) => {
                                    LuaResponse::String("[function]".to_string())
                                }
                                mlua::Value::Thread(_) => {
                                    LuaResponse::String("[thread]".to_string())
                                }
                                mlua::Value::UserData(_) => {
                                    LuaResponse::String("[userdata]".to_string())
                                }
                                mlua::Value::LightUserData(_) => {
                                    LuaResponse::String("[lightuserdata]".to_string())
                                }
                                _ => LuaResponse::Nil,
                            };
                            sync.send(output)
                        }
                        Err(er) => sync.send(LuaResponse::String(er.to_string())),
                    } {
                        Err(e) => {
                            loggy
                                .send((LogType::LuaSysError, format!("com callback err -> {}", e)));
                        }
                        _ => {}
                    };
                }
                LuaTalk::Resize(w, h) => {
                    gui_handle.borrow_mut().resize(w, h);
                }
                LuaTalk::Drop(s) => {
                    let res = lua_ctx
                        .load(&format!("drop('{}')", s))
                        .eval::<mlua::Value>();
                    if let Err(e) = res {
                        if let Some(d) = lua_ctx.inspect_stack(1) {
                            println!("stack {:?}", d.stack());
                            println!("er line is {}", d.curr_line());
                        };
                        async_sender
                            .send((bundle_id, crate::MainCommmand::AsyncError(format!("{}", e))));
                    }
                }
            }

            // if s1 == "load" {
            // if s2 == "_self_destruct" {

            //     #[cfg(feature = "online_capable")]
            //     match closer {
            //         Some(n) => {
            //             // println!("closing 2");
            //             n.send(vec![-99., 0., 0.]);
            //         }
            //         _ => {}
            //     }
            //     break;
            // } else {
            //     if let Err(er) = lua_load(&lua_ctx, &s2) {
            //         loggy.send((LogType::LuaError, er.to_string()));
            //     }
            // }
            // } else {
            //MARK if loop() then path string is the keyboard keys

            // if s1 == "_self_destruct" {
            //     #[cfg(feature = "online_capable")]
            //     match closer {
            //         Some(n) => {
            //             n.send(vec![-99., 0., 0.]);
            //         }
            //         _ => {}
            //     }
            //     break;
            // }

            // TODO load's chunk should call set_name to "main" etc, for better error handling
            // let res = lua_ctx.load(&s1).eval::<mlua::Value>();
            // match channel {
            //     Some(sync) => {
            //         match match res {
            //             Ok(o) => {
            //                 // let output = format!("{:?}", o);
            //                 let output = match o {
            //                     mlua::Value::String(str) => {
            //                         LuaResponse::String(format!("{:?}", str))
            //                     }
            //                     mlua::Value::Integer(i) => LuaResponse::Integer(i),
            //                     mlua::Value::Number(n) => LuaResponse::Number(n),
            //                     mlua::Value::Boolean(b) => LuaResponse::Boolean(b),
            //                     mlua::Value::Table(t) => {
            //                         let mut hash: HashMap<String, String> = HashMap::new();
            //                         for (i, pair) in
            //                             t.pairs::<mlua::Value, mlua::Value>().enumerate()
            //                         {
            //                             if let Ok((k, v)) = pair {
            //                                 if let mlua::Value::String(key) = k {
            //                                     if let mlua::Value::String(val) = v {
            //                                         hash.insert(
            //                                             format!(
            //                                                 "{}",
            //                                                 key.to_str()
            //                                                     .unwrap_or(&i.to_string())
            //                                             ),
            //                                             format!(
            //                                                 "{}",
            //                                                 val.to_str().unwrap_or("")
            //                                             ),
            //                                         );
            //                                     }
            //                                 }
            //                             }
            //                         }
            //                         LuaResponse::Table(hash)
            //                     }
            //                     mlua::Value::Function(_) => {
            //                         LuaResponse::String("[function]".to_string())
            //                     }
            //                     mlua::Value::Thread(_) => {
            //                         LuaResponse::String("[thread]".to_string())
            //                     }
            //                     mlua::Value::UserData(_) => {
            //                         LuaResponse::String("[userdata]".to_string())
            //                     }
            //                     mlua::Value::LightUserData(_) => {
            //                         LuaResponse::String("[lightuserdata]".to_string())
            //                     }
            //                     _ => LuaResponse::Nil,
            //                 };
            //                 sync.send((output, None))
            //             }
            //             Err(er) => sync.send((LuaResponse::String(er.to_string()), None)),
            //         } {
            //             Err(e) => {
            //                 // println!("loop err: {}", e);
            //                 // if !s1.starts_with("loop") {
            //                 loggy.send((
            //                     LogType::LuaSysError,
            //                     format!("lua server communication error occured -> {}", e),
            //                 ));

            //                 // }
            //             }
            //             _ => {}
            //         };
            //     }
            //     _ => {
            //         //=== async functions error handler will debounce since we deal with rapid event looping ===
            //         match res {
            //             Err(e) => {
            //                 debounce_error_string = format!("{}", e);
            //                 debounce_error_counter += 1;
            //                 if debounce_error_counter >= 60 {
            //                     debounce_error_counter = 0;
            //                     match lua_ctx.inspect_stack(1) {
            //                         Some(d) => {
            //                             // d.names().name
            //                             println!("😩😩😩stack {:?}", d.stack());
            //                             println!("er line is {}", d.curr_line());
            //                         }
            //                         _ => {}
            //                     };
            //                     async_sender.send((
            //                         bundle_id,
            //                         crate::MainCommmand::AsyncError(debounce_error_string),
            //                     ));
            //                 }
            //             }
            //             _ => {}
            //         }
            //     }
            // }

            // // updated with our input information, as this is only provided within the game loop, also send out a gui update
            // match bit_in {
            //     Some(b) => {
            //         let mut h = diff_keys_mutex.borrow_mut();

            //         keys_mutex.borrow().iter().enumerate().for_each(|(i, k)| {
            //             h[i] = !k && b.0[i];
            //         });
            //         drop(h);

            //         *keys_mutex.borrow_mut() = b.0;
            //         *mice_mutex.borrow_mut() = b.1;

            //         // only send if a change was made, otherwise the old image is cached on the main thread

            //         async_sender.send((
            //             bundle_id,
            //             crate::MainCommmand::LoopComplete(gui_handle.borrow_mut().send_state()),
            //         ));
            //     }
            //     _ => {}
            // }
            // }

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
    (sender, thread_join)
}

#[cfg(feature = "audio")]
use crate::sound::SoundCommand;
use crate::{
    bundle::{BundleMutations, BundleResources},
    command::MainCommmand,
    error::P64Error,
    log::LogType,
    lua_img::LuaImg,
    pad::Pad,
    pool::{LocalPool, SharedPool},
    types::{ControlState, Script},
    world::{TileCommand, TileResponse},
};
use gilrs::{Axis, Button, Event, EventType, Gilrs};
#[cfg(feature = "puc_lua")]
use mlua::{prelude::LuaError, Lua, Value};
use parking_lot::Mutex;
use silt_lua::{
    gc_arena::Mutation, lua::VM, prelude::Compiler, ExVal, 
};
// use piccolo::{
//     compiler::{self as Compiler, interning::BasicInterner},
//     error::{LuaError, StaticLuaError},
//     lua, meta_ops, AnyCallback, CallbackReturn, Closure, Context, Error, Execution, Executor,
//     FromMultiValue, Fuel, Function, FunctionPrototype, Lua, PrototypeError, Stack, StashedExecutor,
//     StaticError, Value,
// };
#[cfg(feature = "silt")]
use silt_lua::{error::ErrorOut, Lua, Value};
use std::{
    cell::RefCell,
    collections::HashMap,
    error::Error,
    io::{BufRead, Read},
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

pub type LuaHandle = thread::JoinHandle<Result<(), String>>;

// pub enum LuaResponse {
//     String(String),
//     Number(f64),
//     Integer(i32),
//     Boolean(bool),
//     Table(HashMap<String, String>),
//     TableOfTuple(HashMap<String, (String, String)>),
//     Nil,
//     Error(String),
// }
pub type LuaResponse = ExVal;

pub trait ReadSend: Read + Send {}
pub enum LuaTalk {
    AsyncFunc(String),
    Func(String, SyncSender<LuaResponse>),
    Main,
    Loop(ControlState),
    // Load(&'lt mut (dyn Read + Send), SyncSender<LuaResponse>), // DEV try using reader
    Load(Box<Script>, SyncSender<LuaResponse>),
    // AsyncLoad(&'lt mut (dyn Read + Send)),
    AsyncLoad(Box<Script>),
    Resize(u32, u32),
    Die,
    Drop(String),
}

// impl From<Value<'_>> for LuaResponse {
//     fn from(v: Value) -> Self {
//         match v {
//             Value::String(str) => {
//                 let s = str.to_string();
//                 LuaResponse::String(s)
//             }
//             Value::Integer(i) => LuaResponse::Integer(i.try_into().unwrap_or(0)), // TODO margin of error
//             Value::Number(n) => LuaResponse::Number(n),
//             Value::Boolean(b) => LuaResponse::Boolean(b),
//             Value::Function(_) => LuaResponse::String("[function]".to_string()),
//             Value::Thread(_) => LuaResponse::String("[thread]".to_string()),
//             Value::UserData(_) => LuaResponse::String("[userdata]".to_string()),
//             Value::Table(_) => LuaResponse::String("[table]".to_string()),
//             Value::Nil => LuaResponse::Nil,
//         }
//     }
// }

pub struct LuaCore {
    to_lua_tx: Sender<LuaTalk>,
}

impl<'lt> LuaCore {
    /** create new but do not start yet. Channel acts as a placeholder */
    pub fn new(// bundle_id: u8,
        // gui: GuiMorsel,
        // world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        // singer: Sender<SoundPacket>,
        // dangerous: bool,
    ) -> LuaCore {
        let (sender, _) = channel::<LuaTalk>();

        LuaCore { to_lua_tx: sender }
    }

    // pub fn start(
    //     &mut self,
    //     bundle_id: u8,
    //     resources: BundleResources,
    //     world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
    //     pitcher: Sender<MainPacket>,
    //     loggy: Sender<(LogType, String)>,
    //     #[cfg(feature = "audio")] singer: SoundSender,
    //     debug: bool,
    //     dangerous: bool,
    // ) -> LuaHandle {
    //     // self.to_lua_tx = rec;

    //     let lua_handle = start(
    //         self,
    //         bundle_id,
    //         resources,
    //         world_sender,
    //         pitcher,
    //         loggy,
    //         #[cfg(feature = "audio")]
    //         singer,
    //         debug,
    //         dangerous,
    //     );
    //     lua_handle
    // }

    // pub fn get_receiver(&mut self) -> Receiver<LuaTalk<'lt>> {
    //     match self.from_lua_tx.take() {
    //         Some(r) => r,
    //         None => {
    //             let (s, r) = channel::<LuaTalk<'lt>>();
    //             self.to_lua_tx = s;
    //             r
    //         }
    //     }
    // }
    pub fn start(
        &mut self,
        bundle_id: u8,
        shared: SharedPool,
        resources: BundleResources,
        world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        pitcher: Sender<MainPacket>,
        loggy: Sender<(LogType, String)>,
        #[cfg(feature = "audio")] singer: SoundSender,
        debug: bool,
        _dangerous: bool,
    ) -> LuaHandle {
        //     let receiver = self.get_receiver();
        //     Self::_start(
        //         // receiver,
        //         bundle_id,
        //         resources,
        //         world_sender,
        //         pitcher,
        //         loggy,
        //         debug,
        //         dangerous,
        //     )
        // }

        // fn _start(

        //     // switch_board: Arc<RwLock<SwitchBoard>>,
        //     // receiver: Receiver<LuaTalk<'lt>>,
        //     // lua_core: &mut LuaCore<'lt>,
        //     bundle_id: u8,
        //     resources: BundleResources,
        //     world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        //     pitcher: Sender<MainPacket>,
        //     loggy: Sender<(LogType, String)>,
        //     #[cfg(feature = "audio")] singer: SoundSender,
        //     debug: bool,
        //     dangerous: bool,
        // ) -> LuaHandle {

        // self.to_lua_tx = sender;
        // drop(self);
        // let  receiver = match self.from_lua_tx.take() {
        //     Some(r) =>  r,
        //     None => channel::<LuaTalk>().1,
        // };
        // let interner = BasicInterner::default();
        if let Err(e) = loggy.send((LogType::LuaSys, format!("init lua core #{}", bundle_id))) {
            println!("lua log failed: {}", e);
        }
        // let tokio_thread = tokio::spawn(future)
        let (sender, receiver) = channel::<LuaTalk>();
        self.to_lua_tx = sender;

        // let (sender, receiver) = bounded(2);

        // tokio::task::spawn_blocking(move || {

        // bounded(4)
        // let r = crossbeam::scope(|s| {
        //     s.spawn(|_| {
        //         print!("hello");
        //     });
        // });

        // thread::scope(|s| {
        //     s.spawn(|_| {
        //         // Not going to compile because we're trying to borrow `s`,
        //         // which lives *inside* the scope! :(
        //         s.spawn(|_| println!("nested thread"));
        //     });
        // });

        // let thread_join = thread::spawn(move || -> Result<(), String> {
        //     for r in receiver {
        //         match r {
        //             LuaTalk::Load(code, sync) => {}
        //             _ => {}
        //         }
        //     }
        //     Ok(())
        // });

        let thread_join = thread::spawn(move || -> Result<(), String> {
            // let receiver = receiver;
            let thread_closure = || -> Result<(), P64Error> {
                // let reciever = receiver;
                // #[cfg(feature = "online_capable")]
                // let net = Rc::new(RefCell::new(crate::online::Online::new()));
                // #[cfg(not(feature = "online_capable"))]
                // let net: Option<bool> = None;

                // #[cfg(feature = "online_capable")]
                // let netout = net.clone();
                // #[cfg(not(feature = "online_capable"))]
                // let netout: Option<bool> = None;

                let keys = [false; 256];
                let mice = [0.; 13];

                let keys_mutex = Rc::new(RefCell::new(keys));
                let diff_keys_mutex = Rc::new(RefCell::new([false; 256]));
                let mice_mutex = Rc::new(RefCell::new(mice));
                let ent_counter = Rc::new(Mutex::new(2u64));
                let (letters, main_im, sky_im, size) = resources;
                let morsel = crate::gui::GuiMorsel::new(letters, size);

                let mut lua_instance = Lua::new_with_standard();
                let letters = morsel.letters.clone();

                let gui_handle = Rc::new(RefCell::new(morsel));

                lua_instance.enter::<_, Result<(), P64Error>>(move |vm, mc| {
                    let mut local_pool = LocalPool::new();

                    let mut compiler = Compiler::new();

                    if debug {
                        loggy.send((
                            LogType::LuaSys,
                            "new controller connector starting".to_owned(),
                        ))?;
                    }
                    let mut gilrs = Gilrs::new().unwrap();
                    for (_id, gamepad) in gilrs.gamepads() {
                        loggy.send((
                            LogType::LuaSys,
                            format!("gamepad {} is {:?}", gamepad.name(), gamepad.power_info()),
                        ))?;
                    }

                    let pads = Rc::new(RefCell::new(Pad::new()));

                    let async_sender = pitcher.clone();
                    // let mut debounce_error_string = "".to_string();
                    let mut debounce_error_counter = 60;

                    let main_rast = LuaImg::new(
                        bundle_id,
                        main_im.clone(),
                        size[0],
                        size[1],
                        letters.clone(),
                    );

                    let sky_rast =
                        LuaImg::new(bundle_id, sky_im.clone(), size[0], size[1], letters.clone());

                    let (main_val, main_ref) = vm.create_userdata_tuple(mc, main_rast);
                    let (sky_val, sky_ref) = vm.create_userdata_tuple(mc, sky_rast);

                    let mut globals = vm.globals.borrow_mut(mc);
                    globals.set("gui", main_val);
                    globals.set("sky", sky_val);
                    drop(globals);
                    let pong = Box::new((main_ref, sky_ref));

                    async_sender.send((bundle_id, MainCommmand::InitBack(pong)))?;
                    
                    match crate::command::init_lua_sys(
                        vm,
                        mc,
                        bundle_id,
                        pitcher.clone(),
                        world_sender.clone(),
                        Rc::clone(&gui_handle),
                        #[cfg(feature = "audio")]
                        singer,
                        Rc::clone(&keys_mutex),
                        Rc::clone(&diff_keys_mutex),
                        Rc::clone(&mice_mutex),
                        Rc::clone(&pads),
                        Rc::clone(&ent_counter),
                        loggy.clone(),
                        local_pool.clone(),
                    ) {
                        Err(err) => {
                            loggy.send((
                                LogType::LuaSysError,
                                format!("lua com inject fail: {}", err),
                            ))?;
                        }
                        _ => {
                            if debug {
                                loggy.send((
                                    LogType::LuaSys,
                                    "lua commands initialized".to_owned(),
                                ))?;
                            }
                        }
                    }
                    if debug {
                        loggy.send((LogType::LuaSys, "begin lua system listener".to_owned()))?;
                    }
                    let main_lua_func =
                        vm.load_fn(mc, &mut compiler, Some("main"), "main() loop()")?;
                    let loop_lua_func = vm.load_fn(mc, &mut compiler, Some("loop"), "loop()")?;
                    let draw_lua_func = vm.load_fn(mc, &mut compiler, Some("draw"), "draw()")?;
                    let drop_lua_func = vm.load_fn(mc, &mut compiler, Some("drop"), "drop()")?;

                    // let main_ref = Rc::new(RefCell::new(f));
                    for m in &receiver {
                        // let (s1, s2, bit_in, channel) = m;
                        #[cfg(feature = "headed")]
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
                                _ => {} //     EventType::ButtonRepeated(_, _) => todo!(),
                                        //     EventType::ButtonChanged(_, _, _) => todo!(),
                                        //     EventType::Connected => todo!(),
                                        //     EventType::Disconnected => todo!(),
                                        //     EventType::Dropped => todo!(),
                            }
                        }

                        // counter += 1;
                        // if counter > 100000 {
                        //     counter = 0;
                        //     println!("loop");
                        // }
                        match m {
                            LuaTalk::Load(code, sync) => {
                                match run_in_context(
                                    vm,
                                    mc,
                                    Some(&code.name),
                                    &mut code.content.as_bytes(),
                                    &mut compiler,
                                ) {
                                    Err(er) => {
                                        loggy.send((LogType::LuaError, er.to_string()))?;
                                        sync.send(LuaResponse::String(er.to_string()))?;
                                    }
                                    Ok(v) => {
                                        sync.send(v)?;
                                    }
                                } // match run_in_context(vm, Some("load ->"), code){
                                  //     Ok(res)=>,
                                  //     Err(er)=>,
                                  // }
                            }
                            LuaTalk::AsyncLoad(code) => {
                                if let Err(er) = run_in_context(
                                    vm,
                                    mc,
                                    Some(&code.name),
                                    &mut code.content.as_bytes(),
                                    &mut compiler,
                                ) {
                                    loggy.send((LogType::LuaError, er.to_string()))?;
                                }
                            }
                            LuaTalk::Main => {
                                vm.call_fn(mc, Some("main"), main_lua_func, ());

                                // if let Err(e) = res {
                                //     async_sender.send((
                                //         bundle_id,
                                //         MainCommmand::AsyncError(format_error_string(e.to_string())),
                                //     ))?;
                                // }
                            }
                            LuaTalk::Die => {
                                // #[cfg(feature = "online_capable")]
                                // net.borrow_mut().shutdown();
                                break;
                            }
                            LuaTalk::AsyncFunc(_func) => {}
                            LuaTalk::Loop((key_state, mouse_state)) => {
                                vm.call_fn(mc, Some("loop"), loop_lua_func, ())?;

                                local_pool.check_lock(&shared);
                                // &lua_instance.execute(&executor)?; // TODO
                                //=== async functions error handler will debounce since we deal with rapid event looping ===
                                // match res {
                                //     Err(e) => {
                                //         // debounce_error_string = formatError(e);
                                //         debounce_error_counter += 1;
                                //         if debounce_error_counter >= 60 {
                                //             debounce_error_counter = 0;
                                //             async_sender.send((
                                //                 bundle_id,
                                //                 MainCommmand::AsyncError(format_error_string(
                                //                     e.to_string(),
                                //                 )),
                                //             ))?;
                                //         }
                                //     }
                                //     _ => {}
                                // }

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

                                // Check if the gui or sky raster has been modified
                                let mut mutations = BundleMutations::new();
                                mutations.gui = false;
                                mutations.sky = false;

                                // let globals = vm.globals.borrow();
                                // if let Some(gui_val) = globals.get("gui") {
                                //     gui_val.apply_userdata_mut(mc,|img: &mut LuaImg| {
                                //         if img.dirty {
                                //             img.dirty = false;
                                //             mutations.gui = true;
                                //             // Set dirty flag in shared pool
                                //             shared.gui_dirty.replace(true);
                                //         }
                                //         Ok(())
                                //     });
                                // }
                                // if let Some(sky_val) = globals.get("sky") {
                                //     sky_val.apply_userdata_mut(mc,|img: &mut LuaImg| {
                                //         if img.dirty {
                                //             img.dirty = false;
                                //             mutations.sky = true;
                                //             // Set dirty flag in shared pool
                                //             shared.sky_dirty.replace(true);
                                //         }
                                //         Ok(())
                                //     });
                                // }
                                // drop(globals);


                                async_sender.send((
                                    bundle_id,
                                    MainCommmand::LoopComplete(mutations),
                                ))?;
                                local_pool.drop();
                            }
                            LuaTalk::Func(func, sync) => {
                                // TODO load's chunk should call set_name to "main" etc, for better error handling
                                let mut s: &mut (dyn Read + Send) = &mut func.as_bytes();
                                let res =
                                    run_in_context(vm, mc, Some("func ->"), s, &mut compiler)?;
                                // let res = match executor.take_result::<Value>(ctx) {
                                //     Ok(v1) => match v1 {
                                //         Ok(v2) => v2,
                                //         Err(_) => Value::Nil,
                                //     },
                                //     Err(_) => Value::Nil,
                                // };
                                // let output = match o {
                                //     Value::Table(t) => {
                                //         let mut hash: HashMap<String, String> =
                                //             HashMap::new();
                                //         let mut hash2: HashMap<String, (String, String)> =
                                //             HashMap::new();
                                //         // t.0.borrow().entries.
                                //         for (i, (k, v)) in t.iter().enumerate() {
                                //             if let Value::String(key) = k {
                                //                 match v {
                                //                     Value::String(val) => {
                                //                         hash.insert(key, val);
                                //                     }
                                //                     Value::Table(tt) => {
                                //                         let t = tt.borrow();
                                //                         if t.len() == 2 {
                                //                             hash2.insert(
                                //                                 key.to_str()
                                //                                     .unwrap_or(
                                //                                         &i.to_string(),
                                //                                     )
                                //                                     .to_string(),
                                //                                 (
                                //                                     t.get(1).to_string(),
                                //                                     t.get(2).to_string(),
                                //                                 ),
                                //                             );
                                //                         }
                                //                     }
                                //                     _ => {}
                                //                 }
                                //             }
                                //         }
                                //         if hash2.len() > 0 {
                                //             LuaResponse::TableOfTuple(hash2)
                                //         } else {
                                //             LuaResponse::Table(hash)
                                //         }
                                //     }
                                //     Value::Function(_) => {
                                //         LuaResponse::Meta("[function]".to_string())
                                //     }
                                //     // Value::LightUserData(_) => {
                                //     //     LuaResponse::String("[lightuserdata]".to_string())
                                //     // }
                                //     v => v.into(),
                                // };
                                sync.send(res)?

                                //     Err(e) => {
                                //         loggy.send((
                                //             LogType::LuaSysError,
                                //             format!("com callback err -> {}", e),
                                //         ))?;
                                //     }
                                //     _ => {}
                                // };
                            }
                            LuaTalk::Resize(w, h) => {
                                println!("resize {} {}", w, h);
                                // gui_handle.borrow_mut().resize(w, h);
                                // main_rast.borrow_mut().resize(w, h);
                                // sky_rast.borrow_mut().resize(w, h);
                                vm.call_fn(mc, Some("redraw"), draw_lua_func, (w, h));

                                // executor.restart(ctx, draw_lua_func, (w, h));
                                // lua_instance.execute(draw_lua_func)?;
                                // let _ = lua_instance
                                //     .load(&format!("draw({},{})", w, h))
                                //     .eval::<Value>();
                            }
                            LuaTalk::Drop(s) => {
                                let res = vm.call_fn(mc, Some("drop"), drop_lua_func, s);

                                if let Err(e) = res {
                                    async_sender.send((
                                        bundle_id,
                                        MainCommmand::AsyncError(e.to_string()),
                                    ))?;
                                }
                            }
                        }
                    }

                    Ok(())
                })
            };
            match thread_closure() {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("lua ctx failure: {}", e)),
            }
        });
        thread_join
    }

    pub fn func(&self, func: &str) -> Result<LuaResponse, P64Error> {
        let (tx, rx) = sync_channel::<LuaResponse>(0);
        // self.inject(func, &"0", None).0
        self.to_lua_tx.send(LuaTalk::Func(func.to_string(), tx));
        match rx.recv_timeout(Duration::from_millis(4000)) {
            Ok(lua_out) => Ok(lua_out),
            Err(_) => Err(P64Error::ChannelTimeoutError), // TODO it could be either Timeout or
                                                          // Disconnected, is it worth
                                                          // distinguishing?
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

    pub fn load<R>(&self, name: String, reader: &'lt mut R) -> Result<LuaResponse, P64Error>
    where
        R: Read + Send,
    {
        // log("loading script".to_string());
        // self.inject(&"load".to_string(), file, None)

        let (tx, rx) = sync_channel::<LuaResponse>(0);
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap(); // DEV can we get the reader instead?
        match self
            .to_lua_tx
            .send(LuaTalk::Load(Box::new(Script { name, content: buf }), tx))
        {
            Ok(_) => match rx.recv_timeout(Duration::from_millis(10000)) {
                Ok(lua_out) => Ok(lua_out),
                Err(_) => Err(P64Error::ChannelTimeoutError),
            },
            Err(_) => Err(P64Error::ChannelDisconnectedError),
        }
    }

    /** Call resize function with resolution within lua app */
    pub fn resize(&self, w: u32, h: u32) {
        self.to_lua_tx.send(LuaTalk::Resize(w, h));
    }

    pub fn async_load(&self, name: String, reader: &'lt mut (dyn Read + Send)) {
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap(); // DEV can we get the reader instead?
        self.to_lua_tx
            .send(LuaTalk::AsyncLoad(Box::new(Script { name, content: buf })));
    }

    /** Call main function within lua app */
    pub fn call_main(&self) {
        self.to_lua_tx.send(LuaTalk::Main);
    }

    /** Call drop function within lua app */
    pub fn call_drop(&self, s: String) {
        self.to_lua_tx.send(LuaTalk::Drop(s));
    }

    /** Call loop function within lua app */
    pub fn call_loop(&self, bits: ControlState) {
        if let Err(e) = self.to_lua_tx.send(LuaTalk::Loop(bits)) {
            println!("lua loop error: {}", e);
        }
    }

    /** sends kill signal to this lua context thread */
    pub fn die(&self) {
        // self.async_inject(&"_self_destruct".to_string(), None);
        self.to_lua_tx.send(LuaTalk::Die);
    }
}

// fn run_in_context<'gc, 'lt>(
//     ctx: &Context<'gc>,
//     executor: &Executor<'gc>,
//     name: Option<&str>,
//     code: &'lt mut (dyn Read + Send),
// ) -> Result<(), Error<'gc>> {
//     let closure = match Closure::load(*ctx, name, code) {
//         Ok(closure) => closure,
//         Err(err) => {
//             return Err(err.into());
//         }
//     };
//     let function = Function::compose(
//         &ctx,
//         [
//             closure.into(),
//             AnyCallback::from_fn(&ctx, |ctx, _, stack| {
//                 Ok(if stack.is_empty() {
//                     CallbackReturn::Return
//                 } else {
//                     CallbackReturn::Call {
//                         function: meta_ops::call(ctx, ctx.get_global("print"))?,
//                         then: None,
//                     }
//                 })
//             })
//             .into(),
//         ],
//     );
//     // executor.
//     executor.restart(*ctx, function, ());
//
//     Ok(())
// }
fn run_in_context<'gc, 'lt, C>(
    vm: &mut VM<'gc>,
    mc: &Mutation<'gc>,
    name: Option<&str>,
    code: &'lt mut C,
    compiler: &mut Compiler,
) -> Result<ExVal, P64Error>
where
    C: Read + Send + ?Sized,
{
    // TODO optimize this
    let mut s = String::new();
    code.read_to_string(&mut s);

    match vm.build_and_run(mc, name, &s, compiler) {
        Ok(v) => Ok(v),
        Err(er) => Err(er.into()),
    }
}

fn run_initial_code<R>(lua: &mut Lua, compiler: &mut Compiler, mut code: R) -> Result<(), ErrorEnum>
where
    R: ReadSend,
{
    // TODO optimize this
    let mut s = String::new();
    code.read_to_string(&mut s);
    lua.run(Some("initial"), &s, compiler)?;
    // if let Err(e) = lua.run(&s, compiler)? ;
    //     return Err(e);
    // }
    // let executor = lua.try_enter(|ctx| {
    //     let closure = Closure::new(
    //         &ctx,
    //         FunctionPrototype::compile(ctx, "initial", code)?,
    //         Some(ctx.globals()),
    //     )?;
    //     Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
    // })?;
    // lua.execute(&executor)?;
    Ok(())
}

/// Build a lua function to be excuted later
// fn build_function<'a>(
//     ctx: Context<'a>,
//     name: Option<&str>,
//     code: &str,
// ) -> Result<Function<'a>, StaticError> {
//     let closure = match Closure::load(ctx, name, ("return ".to_string() + code).as_bytes()) {
//         Ok(closure) => closure,
//         Err(err) => {
//             if let Ok(closure) = Closure::load(ctx, name, code.as_bytes()) {
//                 closure
//             } else {
//                 return Err(StaticError::Runtime(err.into()));
//             }
//         }
//     };
//     Ok(Function::compose(
//         &ctx,
//         [
//             closure.into(),
//             AnyCallback::from_fn(&ctx, |ctx, _, stack| {
//                 Ok(if stack.is_empty() {
//                     CallbackReturn::Return
//                 } else {
//                     CallbackReturn::Call {
//                         function: meta_ops::call(ctx, ctx.get_global("print"))?,
//                         then: None,
//                     }
//                 })
//             })
//             .into(),
//         ],
//     ))
// }

// pub fn native_function<'a, 'gc, F>(ctx: &Context<'gc>, func: F) -> Result<Value<'gc>, StaticError>
// where
//     F: 'static
//         + Fn(
//             Context<'gc>,
//             Execution<'gc, '_>,
//             Stack<'gc, '_>,
//         ) -> Result<CallbackReturn<'gc>, Error<'gc>>,
// {
//     Ok(AnyCallback::from_fn(ctx, func).into())
// }

// fn build_and_run_function(
//     lua: &mut Lua,
//     name: Option<&str>,
//     code: &str,
// ) -> Result<StashedExecutor, StaticError> {
//     let func = lua.try_enter(|ctx| {
//         let func = build_function(ctx, name, code)?;
//         Ok(ctx.stash(Executor::start(ctx, func, ())))
//     })?;
//     Ok(func)
// }

// pub fn run_code(
//     lua: &mut Lua,
//     executor: &StashedExecutor,
//     name: Option<&str>,
//     code: &str,
// ) -> Result<(), StaticError> {
//     lua.try_enter(|ctx| {
//         let closure = match Closure::load(ctx, name, ("return ".to_string() + code).as_bytes()) {
//             Ok(closure) => closure,
//             Err(err) => {
//                 if let Ok(closure) = Closure::load(ctx, name, code.as_bytes()) {
//                     closure
//                 } else {
//                     return Err(err.into());
//                 }
//             }
//         };
//         let function = Function::compose(
//             &ctx,
//             [
//                 closure.into(),
//                 AnyCallback::from_fn(&ctx, |ctx, _, stack| {
//                     Ok(if stack.is_empty() {
//                         CallbackReturn::Return
//                     } else {
//                         CallbackReturn::Call {
//                             function: meta_ops::call(ctx, ctx.get_global("print"))?,
//                             then: None,
//                         }
//                     })
//                 })
//                 .into(),
//             ],
//         );
//         let executor = ctx.fetch(executor);
//         executor.restart(ctx, function, ());
//         Ok(())
//     })?;
//
//     lua.execute::<()>(executor)
// }

// pub fn execute<'gc>(
//     ctx: Context<'gc>,
//     executor: &Executor<'gc>,
//     name: Option<&str>,
//     code: &str,
// ) -> Result<(), PrototypeError> {
//     // lua.try_run(|ctx| {
//     let closure = match Closure::load(ctx, name, code.as_bytes()) {
//         Ok(closure) => closure,
//         Err(err) => {
//             return Err(err);
//         }
//     };
//
//     let func = Function::from(closure);
//
//     executor.restart(ctx, func, ());
//     Ok(())
//
// }

// fn lua_load<'a, R>(
//     lua: &Lua,
//     executor: &StashedExecutor,
//     interner: &mut BasicInterner,
//     st: R,
// ) -> Result<(), P64Error>
// where
//     R: Read,
// {
//     // let file = piccolo::io::buffered_read(std::fs::File::open("file").unwrap()).unwrap();
//     let chnk = Compiler::parse_chunk(st, interner)?;
//     let bytecode = Compiler::compile_chunk(&chnk, interner)?;
//     // interner.lua.execute(st);
//     // let executor = lua.try_run(|ctx| {
//     //     let closure = Closure::new(
//     //         &ctx,
//     //         FunctionProto::compile(ctx, file)?,
//     //         Some(ctx.state.globals),
//     //     )?;
//     //     Ok(ctx
//     //         .state
//     //         .registry
//     //         .stash(&ctx, Executor::start(ctx, closure.into(), ())))
//     // })?;

//     lua.execute(executor);
//     Ok(())
//     // lua.run(st)
//     // let chunk = lua.load(st);

//     // chunk.exec()
// }

#[cfg(feature = "puc_lua")]
type ErrorEnum = mlua::Error;
#[cfg(feature = "silt")]
type ErrorEnum = ErrorOut;

#[cfg(feature = "picc")]
type ErrorEnum<'a> = LuaError<'a>;

// fn unwrap_err<T>(re: Result<T, ErrorEnum>) -> Result<T, P64Error> {
//     re.map_err(|e| P64Error::LuaRunError(e))
// }
fn unwrap<T>(re: Result<T, ErrorEnum>) -> Result<T, P64Error> {
    // re.map_err(|e|Box::new(P64Error::from(e)))
    re.map_err(|e| P64Error::LuaRunError(Box::new(e)))
}
fn format_error(e: ErrorEnum) -> String {
    format_error_string(e.to_string())
}

fn format_error_string(s: String) -> String {
    // return s;
    let mut cause = "";
    let array = s.split("\n").filter_map(|p| {
        if let Some((_, trace)) = p.split_once("]") {
            if trace.len() > 0 {
                println!("line: {}", trace);
                let parts: Vec<&str> = trace.split(":").collect();
                let plen = parts.len();
                if plen > 2 {
                    let code = parts.get(1).unwrap();
                    let mes = parts.get(2).unwrap();

                    return Some(code.to_string() + &mes.replace("function", "fn"));
                } else if plen == 2 {
                    let mes = parts.get(1).unwrap();

                    return Some(mes.replace("function", "fn"));
                } else if plen == 1 {
                    return Some("?".to_owned() + parts[0]);
                } else {
                    return Some("?".to_owned());
                }
                // return Some(trace);
            }
        } else {
            if p.starts_with("caused by") {
                // return everything after 'caused by:'
                if p.starts_with("caused by: runtime error: ") {
                    cause = p.split_at(26).1;
                } else {
                    cause = p.split_at(10).1;
                }
                return None;
            }
        }
        None
    });
    // we remove the last item as it is just the rust code calling the lua context
    let mut array = array.collect::<Vec<String>>();
    array.pop();

    format!("{} >{}", cause, array.join(" >"))
}

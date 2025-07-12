#[cfg(feature = "audio")]
use crate::sound::SoundCommand;
use crate::{
    bundle::BundleMutations,
    command::MainCommmand,
    error::P64Error,
    log::LogType,
    lua_img::LuaImg,
    pad::Pad,
    pool::{LocalPool, SharedPool},
    types::ControlState,
    world::{TileCommand, TileResponse},
};
use crossbeam::channel::bounded;
use gilrs::{Axis, Button, Event, EventType, Gilrs};
#[cfg(feature = "puc_lua")]
use mlua::{prelude::LuaError, Lua, Value};
use parking_lot::Mutex;
use silt_lua::{lua::VM, prelude::Compiler};
// use piccolo::{
//     compiler::{self as Compiler, interning::BasicInterner},
//     error::{LuaError, StaticLuaError},
//     lua, meta_ops, AnyCallback, CallbackReturn, Closure, Context, Error, Execution, Executor,
//     FromMultiValue, Fuel, Function, FunctionPrototype, Lua, PrototypeError, Stack, StashedExecutor,
//     StaticError, Value,
// };
#[cfg(feature = "silt")]
use silt_lua::prelude::{Lua, LuaError, Value};
use std::{
    cell::RefCell,
    collections::HashMap,
    io::{BufRead, BufReader, Read},
    rc::Rc,
    sync::{
        mpsc::{channel, sync_channel, Receiver, Sender, SyncSender},
        Arc,
    },
    thread,
    time::Duration,
};

#[cfg(feature = "audio")]
pub type SoundSender = Sender<SoundCommand>;
#[cfg(not(feature = "audio"))]
pub type SoundSender = ();

pub type MainPacket = (u8, MainCommmand);

pub type LuaHandle = thread::JoinHandle<Result<(), String>>;

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

pub trait ReadSend: Read + Send {}
pub enum LuaTalk {
    AsyncFunc(String),
    Func(String, SyncSender<LuaResponse>),
    Main,
    Loop(ControlState),
    // Load(&'lt mut (dyn Read + Send), SyncSender<LuaResponse>), // DEV try using reader
    Load(String, SyncSender<LuaResponse>),
    // AsyncLoad(&'lt mut (dyn Read + Send)),
    AsyncLoad(String),
    Resize(u32, u32),
    Die,
    Drop(String),
}

impl From<Value<'_>> for LuaResponse {
    fn from(v: Value) -> Self {
        match v {
            Value::String(str) => {
                let s = str.to_string();
                LuaResponse::String(s)
            }
            Value::Integer(i) => LuaResponse::Integer(i.try_into().unwrap_or(0)), // TODO margin of error
            Value::Number(n) => LuaResponse::Number(n),
            Value::Boolean(b) => LuaResponse::Boolean(b),
            Value::Function(_) => LuaResponse::String("[function]".to_string()),
            Value::Thread(_) => LuaResponse::String("[thread]".to_string()),
            Value::UserData(_) => LuaResponse::String("[userdata]".to_string()),
            Value::Table(_) => LuaResponse::String("[table]".to_string()),
            Value::Nil => LuaResponse::Nil,
        }
    }
}

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
        let (sender, receiver) = channel::<LuaTalk>();

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
        world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        pitcher: Sender<MainPacket>,
        loggy: Sender<(LogType, String)>,
        #[cfg(feature = "audio")] singer: SoundSender,
        debug: bool,
        dangerous: bool,
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
            let receiver = receiver;
            let thread_result = || -> Result<(), P64Error> {
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
                // let (letters, main_im, sky_im, size) = resources;
                // let morsel = crate::gui::GuiMorsel::new(letters, size);
                // let main_rast2 = Rc::new(RefCell::new(LuaImg::new(
                //     bundle_id,
                //     main_im,
                //     size[0],
                //     size[1],
                //     morsel.letters.clone(),
                // )));
                // let sky_rast2 = Rc::new(RefCell::new(LuaImg::new(
                //     bundle_id,
                //     sky_im,
                //     size[0],
                //     size[1],
                //     morsel.letters.clone(),
                // )));
                // morsel.sky
                //     let im = GuiMorsel::new_image(w, h);
                // let gui_handle = Rc::new(RefCell::new(morsel));

                // TODO safety?
                // let lua_ctx = if false {
                //     unsafe { Lua::unsafe_new_with(mlua::StdLib::ALL, mlua::LuaOptions::new()) }
                // } else {
                //     Lua::new()
                // };
                //

                let compiler = Compiler::new();

                let mut lua_instance = Lua::new_with_standard();
                // let interner = BasicInterner::default();
                // let thread = lua_instance.enter(|ctx| {
                //     let globals = &ctx.state.globals;

                //     // ctx.state.registry.stash(&ctx, Thread::new(&ctx)
                // });

                let mut local_pool = LocalPool::new();

                lua_instance.enter(|ctx, mc| {

                    // let executor = Executor::new(ctx);

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
                    let gui_link = Rc::new(RefCell::new(shared.gui.borrow_mut()));
                    match crate::command::init_lua_sys(
                        &ctx,
                        bundle_id,
                        pitcher,
                        world_sender,
                        // Rc::clone(&gui_handle),
                        // Rc::clone(&main_rast),
                        // Rc::clone(&sky_rast),
                        #[cfg(feature = "audio")]
                        singer,
                        Rc::clone(&keys_mutex),
                        Rc::clone(&diff_keys_mutex),
                        Rc::clone(&mice_mutex),
                        Rc::clone(&pads),
                        Rc::clone(&ent_counter),
                        loggy.clone(),
                        local_pool.clone(), // shared,
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
                    let main_lua_func = ctx.load_fn(mc, &mut compiler, Some("main".to_owned()),"main() loop()")?;
                    let loop_lua_func = ctx.load_fn(mc, &mut compiler, Some("loop".to_owned()),"loop()")?;
                    let draw_lua_func = ctx.load_fn(mc, &mut compiler, Some("draw".to_owned()),"draw()")?;
                    let drop_lua_func = ctx.load_fn(mc, &mut compiler, Some("drop".to_owned()),"drop()")?;

                    // let main_ref = Rc::new(RefCell::new(f));
                    for m in receiver {
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
                        _ => {}
                        //     EventType::ButtonRepeated(_, _) => todo!(),
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
                                // if let Err(er) = run_in_context(
                                //     &ctx,
                                //     Some("load ->"),
                                //     &mut code.as_bytes(),
                                // ) {
                                //     loggy.send((LogType::LuaError, er.to_string()))?;
                                //     sync.send(LuaResponse::String(er.to_string()))?;
                                // } else {
                                //     let res = match executor.take_result::<Value>(ctx) {
                                //         Ok(v1) => match v1 {
                                //             Ok(v2) => v2,
                                //             Err(_) => Value::Nil,
                                //         },
                                //         Err(_) => Value::Nil,
                                //     };
                                //     sync.send(res.into())?;
                                // }
                                match run_in_context(vm, name, code){
                                    Ok(res)=>,
                                    Err(er)=>,
                                }
                            }
                            LuaTalk::AsyncLoad(code) => {
                                if let Err(er) = run_in_context(
                                    &ctx,
                                    &executor,
                                    Some("async load->"),
                                    &mut code.as_bytes(),
                                ) {
                                    loggy.send((LogType::LuaError, er.to_string()))?;
                                }
                            }
                            LuaTalk::Main => {
                                executor.restart(ctx, main_lua_func, ());

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
                                executor.restart(ctx, loop_lua_func, ());

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
                                // mouse_state;

                                // only send if a change was made, otherwise the old image is cached on the main thread
                                // let mut m = main_rast.borrow_mut();
                                // let mm = if m.dirty {
                                //     m.dirty = false;
                                //     Some(m.image.clone())
                                // } else {
                                //     None
                                // };

                                // let mut s = sky_rast.borrow_mut();
                                // let ss = if s.dirty {
                                //     s.dirty = false;
                                //     Some(s.image.clone())
                                // } else {
                                //     None
                                // };

                                // if ss.is_some() || mm.is_some() {
                                async_sender.send((
                                    bundle_id,
                                    MainCommmand::LoopComplete(BundleMutations::new()),
                                ))?;
                                local_pool.drop();
                                // }
                            }
                            LuaTalk::Func(func, sync) => {
                                // TODO load's chunk should call set_name to "main" etc, for better error handling
                                let mut s: &mut (dyn Read + Send) = &mut func.as_bytes();
                                run_in_context(&ctx, &executor, Some("func ->"), s)?;
                                let res = executor.take_result::<Value>(ctx)?;
                                // let res = match executor.take_result::<Value>(ctx) {
                                //     Ok(v1) => match v1 {
                                //         Ok(v2) => v2,
                                //         Err(_) => Value::Nil,
                                //     },
                                //     Err(_) => Value::Nil,
                                // };
                                match match res {
                                    Ok(o) => {
                                        // let output = format!("{:?}", o);
                                        let output = match o {
                                            Value::String(str) => {
                                                let s = str.to_str().unwrap_or(&"").to_string();
                                                LuaResponse::String(s)
                                            }
                                            Value::Integer(i) => LuaResponse::Integer(
                                                i32::try_from(i).unwrap_or(i32::MAX),
                                            ),
                                            Value::Number(n) => {
                                                // println!("func back {:?}", n);
                                                LuaResponse::Number(n)
                                            }
                                            Value::Boolean(b) => LuaResponse::Boolean(b),
                                            Value::Table(t) => {
                                                let mut hash: HashMap<String, String> =
                                                    HashMap::new();
                                                let mut hash2: HashMap<String, (String, String)> =
                                                    HashMap::new();
                                                // t.0.borrow().entries.
                                                for (i, (k, v)) in t.iter().enumerate() {
                                                    if let Value::String(key) = k {
                                                        match v {
                                                            Value::String(val) => {
                                                                let hash_key = key
                                                                    .to_str()
                                                                    .unwrap_or(&i.to_string())
                                                                    .to_string();
                                                                hash.insert(
                                                                    hash_key,
                                                                    val.to_str()
                                                                        .unwrap_or("")
                                                                        .to_string(),
                                                                );
                                                            }
                                                            Value::Table(tt) => {
                                                                if tt.length() == 2 {
                                                                    hash2.insert(
                                                                        key.to_str()
                                                                            .unwrap_or(
                                                                                &i.to_string(),
                                                                            )
                                                                            .to_string(),
                                                                        (
                                                                            tt.get(ctx, 1)
                                                                                .to_string(),
                                                                            tt.get(ctx, 2)
                                                                                .to_string(),
                                                                        ),
                                                                    );
                                                                }
                                                            }
                                                            _ => {}
                                                        }
                                                    }
                                                }
                                                if hash2.len() > 0 {
                                                    LuaResponse::TableOfTuple(hash2)
                                                } else {
                                                    LuaResponse::Table(hash)
                                                }
                                            }
                                            Value::Function(_) => {
                                                LuaResponse::String("[function]".to_string())
                                            }
                                            Value::Thread(_) => {
                                                LuaResponse::String("[thread]".to_string())
                                            }
                                            Value::UserData(_) => {
                                                LuaResponse::String("[userdata]".to_string())
                                            }
                                            // Value::LightUserData(_) => {
                                            //     LuaResponse::String("[lightuserdata]".to_string())
                                            // }
                                            _ => LuaResponse::Nil,
                                        };
                                        sync.send(output)
                                    }
                                    Err(er) => sync.send(LuaResponse::String(er.to_string())),
                                } {
                                    Err(e) => {
                                        loggy.send((
                                            LogType::LuaSysError,
                                            format!("com callback err -> {}", e),
                                        ))?;
                                    }
                                    _ => {}
                                };
                            }
                            LuaTalk::Resize(w, h) => {
                                println!("resize {} {}", w, h);
                                // gui_handle.borrow_mut().resize(w, h);
                                // main_rast.borrow_mut().resize(w, h);
                                // sky_rast.borrow_mut().resize(w, h);
                                executor.restart(ctx, draw_lua_func, (w, h));
                                // lua_instance.execute(draw_lua_func)?;
                                // let _ = lua_instance
                                //     .load(&format!("draw({},{})", w, h))
                                //     .eval::<Value>();
                            }
                            LuaTalk::Drop(s) => {
                                executor.restart(ctx, drop_lua_func, s);
                                let res = executor.take_result::<Value>(ctx);

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
                })?;
                Ok(())
            }();
            match thread_result {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("lua ctx failure: {}", e)),
            }
        });
        thread_join
    }

    pub fn func<'a>(&self, func: &str) -> LuaResponse {
        let (tx, rx) = sync_channel::<LuaResponse>(0);
        // self.inject(func, &"0", None).0
        self.to_lua_tx.send(LuaTalk::Func(func.to_string(), tx));
        match rx.recv_timeout(Duration::from_millis(4000)) {
            Ok(lua_out) => lua_out,
            Err(e) => LuaResponse::Error(format!("No/slow response from lua -> {}", e)),
        }
    }

    pub fn halt_until_complete<'gc, R: FromMultiValue<'gc>>(
        &self,
        ctx: &Context<'gc>,
        executor: &Executor<'gc>,
    ) -> Result<R, StaticError> {
        const FUEL_PER_GC: i32 = 4096;
        let c = *ctx;
        loop {
            let mut fuel = Fuel::with(FUEL_PER_GC);
            if executor.step(c, &mut fuel) {
                break;
            }
        }

        match executor.take_result::<R>(c) {
            Ok(v1) => match v1 {
                Ok(v2) => Ok(v2),
                Err(e) => Err(e.into_static()),
            },
            Err(e) => Err(StaticError::Runtime(e.into())),
        }
        // .map_err(piccolo::error::Error::into_static);
        // match executor.take_result::<R>(c) {
        //     Ok(v1) => match v1 {
        //         Ok(v2) => Ok(v2),
        //         Err(e) =>
        //     },
        //     Err(_) => Err(StaticError::from("lua error")),
        // }
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

    pub fn load<R>(&self, reader: &'lt mut R) -> LuaResponse
    where
        R: Read + Send,
    {
        // log("loading script".to_string());
        // self.inject(&"load".to_string(), file, None)

        let (tx, rx) = sync_channel::<LuaResponse>(0);
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap(); // DEV can we get the reader instead?
        match self.to_lua_tx.send(LuaTalk::Load(buf, tx)) {
            Ok(_) => match rx.recv_timeout(Duration::from_millis(10000)) {
                Ok(lua_out) => lua_out,
                Err(e) => LuaResponse::Error(format!("No / >10s response from lua -> {}", e)),
            },
            Err(e) => LuaResponse::Error(format!("Cannot speak to lua: {}", e)),
        }
    }

    /** Call resize function with resolution within lua app */
    pub fn resize(&self, w: u32, h: u32) {
        self.to_lua_tx.send(LuaTalk::Resize(w, h));
    }

    pub fn async_load(&self, reader: &'lt mut (dyn Read + Send)) {
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap(); // DEV can we get the reader instead?
        self.to_lua_tx.send(LuaTalk::AsyncLoad(buf));
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
fn run_in_context<'gc, 'lt>(
    vm: &mut VM<'gc>,
    name: Option<&str>,
    code: &'lt mut (dyn Read + Send),
) -> Result<(), Error<'gc>> {
    vm.load()
    
}

fn run_initial_code<R>(lua: &mut Lua, code: R) -> Result<(), StaticError>
where
    R: ReadSend,
{
    let executor = lua.try_enter(|ctx| {
        let closure = Closure::new(
            &ctx,
            FunctionPrototype::compile(ctx, "initial", code)?,
            Some(ctx.globals()),
        )?;
        Ok(ctx.stash(Executor::start(ctx, closure.into(), ())))
    })?;
    lua.execute(&executor)?;
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
type ErrorOut = mlua::Error;
#[cfg(feature = "silt")]
type ErrorOut = silt_lua::prelude::LuaError;

#[cfg(feature = "picc")]
type ErrorOut<'a> = LuaError<'a>;

fn format_error(e: ErrorOut) -> String {
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

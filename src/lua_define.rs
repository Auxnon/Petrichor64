#[cfg(feature = "audio")]
use crate::sound::SoundCommand;
use crate::{
    bundle::BundleResources,
    command::MainCommmand,
    error::P64Error,
    log::LogType,
    lua_img::LuaImg,
    pad::Pad,
    types::ControlState,
    world::{TileCommand, TileResponse},
};
use gilrs::{Axis, Button, Event, EventType, Gilrs};
#[cfg(feature = "puc_lua")]
use mlua::{prelude::LuaError, Lua, Value};
use parking_lot::Mutex;
use piccolo::{
    compiler::{self as Compiler, interning::BasicInterner},
    error::{LuaError, StaticLuaError},
    meta_ops, AnyCallback, CallbackReturn, Closure, Context, Error, Executor, Function,
    FunctionProto, Lua, StashedExecutor, StaticError, Value,
};
#[cfg(feature = "silt")]
use silt_lua::prelude::{Lua, LuaError, Value};
use std::{
    cell::RefCell,
    collections::HashMap,
    io::{BufRead, BufReader, Read},
    rc::Rc,
    sync::{
        mpsc::{channel, sync_channel, Sender, SyncSender},
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

pub enum LuaTalk<'lt> {
    AsyncFunc(String),
    Func(String, SyncSender<LuaResponse>),
    Main,
    Loop(ControlState),
    Load(String, SyncSender<LuaResponse>),
    AsyncLoad(&'lt str),
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

pub struct LuaCore<'lt> {
    to_lua_tx: Sender<LuaTalk<'lt>>,
}

impl<'lt> LuaCore<'lt> {
    /** create new but do not start yet. Channel acts as a placeholder */
    pub fn new(// bundle_id: u8,
        // gui: GuiMorsel,
        // world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        // singer: Sender<SoundPacket>,
        // dangerous: bool,
    ) -> LuaCore<'lt> {
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
        #[cfg(feature = "audio")] singer: SoundSender,
        debug: bool,
        dangerous: bool,
    ) -> LuaHandle {
        let (rec, lua_handle) = start(
            bundle_id,
            resources,
            world_sender,
            pitcher,
            loggy,
            #[cfg(feature = "audio")]
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

    /** Call resize function with resolution within lua app */
    pub fn resize(&self, w: u32, h: u32) {
        self.to_lua_tx.send(LuaTalk::Resize(w, h));
    }

    pub fn async_load(&self, code: &str) {
        self.to_lua_tx.send(LuaTalk::AsyncLoad(code));
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

fn run_in_context<'gc>(
    ctx: &Context<'_>,
    executor: &Executor<'gc>,
    code: &str,
) -> Result<(), Error<'gc>> {
    let closure = match Closure::load(*ctx, code.as_bytes()) {
        Ok(closure) => closure,
        Err(err) => {
            return Err(err.into());
        }
    };
    let function = Function::compose(
        &ctx,
        [
            closure.into(),
            AnyCallback::from_fn(&ctx, |ctx, _, stack| {
                Ok(if stack.is_empty() {
                    CallbackReturn::Return
                } else {
                    CallbackReturn::Call {
                        function: meta_ops::call(ctx, ctx.state.globals.get(ctx, "print"))?,
                        then: None,
                    }
                })
            })
            .into(),
        ],
    );
    // executor.
    executor.restart(*ctx, function, ());

    Ok(())
}

fn run_initial_code<R>(lua: &mut Lua, code: R) -> Result<(), StaticError>
where
    R: Read,
{
    let executor = lua.try_run(|ctx| {
        let closure = Closure::new(
            &ctx,
            FunctionProto::compile(ctx, code)?,
            Some(ctx.state.globals),
        )?;
        Ok(ctx
            .state
            .registry
            .stash(&ctx, Executor::start(ctx, closure.into(), ())))
    })?;
    lua.execute(&executor)?;
    Ok(())
}

/// Build a lua function to be excuted later
fn build_function<'a>(
    lua: &mut Lua,
    ctx: Context<'_>,
    code: &str,
) -> Result<Function<'a>, StaticError> {
    let closure = match Closure::load(ctx, ("return ".to_string() + code).as_bytes()) {
        Ok(closure) => closure,
        Err(err) => {
            if let Ok(closure) = Closure::load(ctx, code.as_bytes()) {
                closure
            } else {
                return Err(StaticError::Runtime(err.into()));
            }
        }
    };
    Ok(Function::compose(
        &ctx,
        [
            closure.into(),
            AnyCallback::from_fn(&ctx, |ctx, _, stack| {
                Ok(if stack.is_empty() {
                    CallbackReturn::Return
                } else {
                    CallbackReturn::Call {
                        function: meta_ops::call(ctx, ctx.state.globals.get(ctx, "print"))?,
                        then: None,
                    }
                })
            })
            .into(),
        ],
    ))
}

fn build_and_run_function(lua: &mut Lua, code: &str) -> Result<StashedExecutor, StaticError> {
    let func = lua.try_run(|ctx| {
        let func = build_function(lua, ctx, code)?;
        Ok(ctx
            .state
            .registry
            .stash(&ctx, Executor::start(ctx, func, ())))
    })?;
    Ok(func)
}

fn run_code(lua: &mut Lua, executor: &StashedExecutor, code: &str) -> Result<(), StaticError> {
    lua.try_run(|ctx| {
        let closure = match Closure::load(ctx, ("return ".to_string() + code).as_bytes()) {
            Ok(closure) => closure,
            Err(err) => {
                if let Ok(closure) = Closure::load(ctx, code.as_bytes()) {
                    closure
                } else {
                    return Err(err.into());
                }
            }
        };
        let function = Function::compose(
            &ctx,
            [
                closure.into(),
                AnyCallback::from_fn(&ctx, |ctx, _, stack| {
                    Ok(if stack.is_empty() {
                        CallbackReturn::Return
                    } else {
                        CallbackReturn::Call {
                            function: meta_ops::call(ctx, ctx.state.globals.get(ctx, "print"))?,
                            then: None,
                        }
                    })
                })
                .into(),
            ],
        );
        let executor = ctx.state.registry.fetch(executor);
        executor.restart(ctx, function, ());
        Ok(())
    })?;

    lua.execute::<()>(executor)
}

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

fn start<'lt>(
    // switch_board: Arc<RwLock<SwitchBoard>>,
    bundle_id: u8,
    resources: BundleResources,
    world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
    pitcher: Sender<MainPacket>,
    loggy: Sender<(LogType, String)>,
    #[cfg(feature = "audio")] singer: SoundSender,
    debug: bool,
    dangerous: bool,
) -> (Sender<LuaTalk<'lt>>, LuaHandle) {
    let (sender, reciever) = channel::<LuaTalk>();
    if let Err(e) = loggy.send((LogType::LuaSys, format!("init lua core #{}", bundle_id))) {
        println!("lua log failed: {}", e);
    }
    let thread_join = thread::spawn(move || -> Result<(), String> {
        let thread_result = || -> Result<(), P64Error> {
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
            let main_rast = Rc::new(RefCell::new(LuaImg::new(
                bundle_id,
                main_im,
                size[0],
                size[1],
                morsel.letters.clone(),
            )));
            let sky_rast = Rc::new(RefCell::new(LuaImg::new(
                bundle_id,
                sky_im,
                size[0],
                size[1],
                morsel.letters.clone(),
            )));
            // morsel.sky
            //     let im = GuiMorsel::new_image(w, h);
            let gui_handle = Rc::new(RefCell::new(morsel));

            // TODO safety?
            // let lua_ctx = if false {
            //     unsafe { Lua::unsafe_new_with(mlua::StdLib::ALL, mlua::LuaOptions::new()) }
            // } else {
            //     Lua::new()
            // };

            let lua_instance = Lua::core();
            let interner = BasicInterner::default();
            let thread = lua_instance.run(|ctx| {
                let globals = &ctx.state.globals;

                // ctx.state.registry.stash(&ctx, Thread::new(&ctx)
            });

            lua_instance.try_run(|ctx| {
                let globals = &ctx.state.globals;

                // let closure = Closure::new(
                //     &ctx,
                //     FunctionProto::compile(ctx, initial_code)?,
                //     Some(ctx.state.globals),
                // )?;

                let executor = Executor::new(ctx);

                // let executor = lua_instance.try_run(|ctx| {
                //     let closure = Closure::new(
                //         &ctx,
                //         FunctionProto::compile(ctx, "print('hello')".as_bytes())?,
                //         Some(ctx.state.globals),
                //     )?;

                //     Ok(ctx
                //         .state
                //         .registry
                //         .stash(&ctx, Executor::start(ctx, closure.into(), ())))
                // })?;
                // // executor.restart(&lua_instance, || {
                // //     // rust things
                // // })?;

                // lua_instance.execute(&executor)?;

                // let executor = lua_runner.run(|ctx| {
                //     let executor = Executor::new(&ctx);
                //     executor.restart(ctx, ||{

                //         // rust things
                //     }, params

                // });
                // ctx.state.registry.stash(&ctx, executor)
                // lua_ctx.load_from_std_lib(mlua::StdLib::DEBUG);
                // lua_ctx.sa

                // let globals = lua_ctx.

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
                match crate::command::init_lua_sys(
                    &lua_instance,
                    &ctx,
                    &globals,
                    bundle_id,
                    pitcher,
                    world_sender,
                    Rc::clone(&gui_handle),
                    Rc::clone(&main_rast),
                    Rc::clone(&sky_rast),
                    #[cfg(feature = "audio")]
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
                            format!("lua com inject fail: {}", err),
                        ))?;
                    }
                    _ => {
                        if debug {
                            loggy.send((LogType::LuaSys, "lua commands initialized".to_owned()))?;
                        }
                    }
                }
                if debug {
                    loggy.send((LogType::LuaSys, "begin lua system listener".to_owned()))?;
                }
                // let mut counter = 0;
                let main_lua_func = build_and_run_function(&mut lua_instance, "main() loop()")?;
                // let main_lua_func = lua_instance.load("main() loop()").into_function()?;
                let loop_lua_func = build_function(&mut lua_instance, ctx, "loop()")?;
                let draw_lua_func = build_function(&mut lua_instance, ctx, "draw()")?;
                let drop_lua_func = build_function(&mut lua_instance, ctx, "drop()")?;

                // let main_ref = Rc::new(RefCell::new(f));
                for m in reciever {
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
                            if let Err(er) = run_in_context(&ctx, &executor, &code) {
                                loggy.send((LogType::LuaError, er.to_string()))?;
                                sync.send(LuaResponse::String(er.to_string()))?;
                            } else {
                                let res = match executor.take_result::<Value>(ctx) {
                                    Ok(v1) => match v1 {
                                        Ok(v2) => v2,
                                        Err(_) => Value::Nil,
                                    },
                                    Err(_) => Value::Nil,
                                };
                                sync.send(res.into())?;
                            }
                        }
                        LuaTalk::AsyncLoad(code) => {
                            if let Err(er) = run_in_context(&ctx, &executor, &code) {
                                loggy.send((LogType::LuaError, er.to_string()))?;
                            }
                        }
                        LuaTalk::Main => {
                            lua_instance.execute(&main_lua_func)?;

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
                            let mut m = main_rast.borrow_mut();
                            let mm = if m.dirty {
                                m.dirty = false;
                                Some(m.image.clone())
                            } else {
                                None
                            };

                            let mut s = sky_rast.borrow_mut();
                            let ss = if s.dirty {
                                s.dirty = false;
                                Some(s.image.clone())
                            } else {
                                None
                            };

                            // if ss.is_some() || mm.is_some() {
                            async_sender.send((bundle_id, MainCommmand::LoopComplete((mm, ss))))?;
                            // }
                        }
                        LuaTalk::Func(func, sync) => {
                            // TODO load's chunk should call set_name to "main" etc, for better error handling
                            run_in_context(&ctx, &executor, &func)?;
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
                                        Value::Integer(i) => LuaResponse::Integer(i),
                                        Value::Number(n) => {
                                            // println!("func back {:?}", n);
                                            LuaResponse::Number(n)
                                        }
                                        Value::Boolean(b) => LuaResponse::Boolean(b),
                                        Value::Table(t) => {
                                            let mut hash: HashMap<String, String> = HashMap::new();
                                            let mut hash2: HashMap<String, (String, String)> =
                                                HashMap::new();
                                            // t.0.borrow().entries.
                                            for (i, pair) in t.pairs::<Value, Value>().enumerate() {
                                                if let Ok((k, v)) = pair {
                                                    if let Value::String(key) = k {
                                                        match v {
                                                            Value::String(val) => {
                                                                hash.insert(
                                                                    format!(
                                                                        "{}",
                                                                        key.to_str().unwrap_or(
                                                                            &i.to_string()
                                                                        )
                                                                    ),
                                                                    format!(
                                                                        "{}",
                                                                        val.to_str().unwrap_or("")
                                                                    ),
                                                                );
                                                            }
                                                            Value::Table(tt) => {
                                                                if tt.raw_len() == 2 {
                                                                    hash2.insert(
                                                                        format!(
                                                                            "{}",
                                                                            key.to_str().unwrap_or(
                                                                                &i.to_string()
                                                                            )
                                                                        ),
                                                                        (
                                                                            tt.get::<_, String>(
                                                                                ctx, 1,
                                                                            )
                                                                            .unwrap_or(
                                                                                "".to_owned(),
                                                                            ),
                                                                            tt.get::<_, String>(
                                                                                ctx, 2,
                                                                            )
                                                                            .unwrap_or(
                                                                                "".to_owned(),
                                                                            ),
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
                                        Value::Function(_) => {
                                            LuaResponse::String("[function]".to_string())
                                        }
                                        Value::Thread(_) => {
                                            LuaResponse::String("[thread]".to_string())
                                        }
                                        Value::UserData(_) => {
                                            LuaResponse::String("[userdata]".to_string())
                                        }
                                        Value::LightUserData(_) => {
                                            LuaResponse::String("[lightuserdata]".to_string())
                                        }
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
                            gui_handle.borrow_mut().resize(w, h);
                            main_rast.borrow_mut().resize(w, h);
                            sky_rast.borrow_mut().resize(w, h);
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
                                async_sender
                                    .send((bundle_id, MainCommmand::AsyncError(e.to_string())))?;
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
    (sender, thread_join)
}

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

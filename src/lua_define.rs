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
use gilrs::Gilrs;
#[cfg(feature = "headed")]
use gilrs::{Axis, Button, Event, EventType};
#[cfg(feature = "puc_lua")]
use mlua::{prelude::LuaError, Lua, Value};
use parking_lot::Mutex;
use silt_lua::{gc_arena::Mutation, lua::VM, prelude::Compiler, userdata::WeakWrapper, ExVal};
// use piccolo::{
//     compiler::{self as Compiler, interning::BasicInterner},
//     error::{LuaError, StaticLuaError},
//     lua, meta_ops, AnyCallback, CallbackReturn, Closure, Context, Error, Execution, Executor,
//     FromMultiValue, Fuel, Function, FunctionPrototype, Lua, PrototypeError, Stack, StashedExecutor,
//     StaticError, Value,
// };
use colored::Colorize;
#[cfg(feature = "silt")]
use silt_lua::{error::ErrorOut, Lua};
use std::{
    cell::RefCell,
    io::Read,
    rc::Rc,
    sync::mpsc::{channel, sync_channel, RecvTimeoutError, Sender, SyncSender},
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
    Die(SyncSender<()>),
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

/// Format a P64Error, rendering Lua run errors as source snippets. The erroring
/// source is looked up in `scripts` by the index silt stamped onto the error
/// (via ErrorOut.source_index → LuaRunError). A missing/untracked index (e.g.
/// `usize::MAX`) degrades to the plain error message.
fn error_string(e: P64Error, scripts: &[Option<String>]) -> String {
    if let P64Error::LuaRunError(errs, idx) = e {
        let source = scripts.get(idx).and_then(|s| s.as_deref());
        errs.iter()
            .map(|e| match source {
                Some(s) => e.snippet(s),
                None => e.to_string(),
            })
            .collect::<Vec<_>>()
            .join("; ")
    } else {
        e.to_string()
    }
}

/// Store `source` at `index` in the sparse `scripts` vec, growing it as needed.
/// One owned copy per distinct compiled source; used to resolve snippets later.
fn store_script(scripts: &mut Vec<Option<String>>, index: usize, source: &str) {
    if index == usize::MAX {
        return;
    }
    if index >= scripts.len() {
        scripts.resize(index + 1, None);
    }
    scripts[index] = Some(source.to_owned());
}

pub struct LuaCore {
    to_lua_tx: Option<Sender<LuaTalk>>,
}

impl<'lt> LuaCore {
    /** create new but do not start yet. Channel acts as a placeholder */
    pub fn new(// bundle_id: u8,
        // gui: GuiMorsel,
        // world_sender: Sender<(TileCommand, SyncSender<TileResponse>)>,
        // singer: Sender<SoundPacket>,
        // dangerous: bool,
    ) -> LuaCore {
        // let (sender, _) = channel::<LuaTalk>();

        LuaCore { to_lua_tx: None }
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
        /* Engine-marked overlay? Gates the `app.*` table on the VM being built below,
        so it must arrive before the first line of Lua runs. */
        privileged: bool,
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

        // TODO do we need to track this? if self.to_lua_tx.is_some() {}

        if let Err(e) = loggy.send((LogType::LuaSys, format!("init lua core #{}", bundle_id))) {
            println!("lua log failed: {}", e);
        }

        // let tokio_thread = tokio::spawn(future)
        let (sender, receiver) = channel::<LuaTalk>();
        self.to_lua_tx = Some(sender);

        // tokio::task::spawn_blocking(move || {

        // bounded(4)
        // let r = crossbeam::scope(|s| {
        //     s.spawn(|_| {
        //         print!("hello");
        //     });
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

                let ent_counter = Rc::new(Mutex::new(2u64));
                let (letters, main_im, sky_im, size) = resources;
                let morsel = crate::gui::GuiMorsel::new(letters, size);

                let mut lua_instance = Lua::new_with_standard();
                let letters = morsel.letters.clone();

                let gui_handle = Rc::new(RefCell::new(morsel));

                lua_instance.enter::<_, Result<(), P64Error>>(move |vm, mc| {
                    let mut local_pool = LocalPool::new();

                    let mut compiler = Compiler::new();

                    // Declared inside enter so they're closure-locals (movable
                    // into ctx below), not captures of this FnMut closure.
                    // `scripts`: sparse store of compiled sources indexed by
                    // silt's source_index, read at error time for snippets.
                    let scripts: Vec<Option<String>> = Vec::new();
                    let keys_mutex = Rc::new(RefCell::new([false; 256]));
                    let diff_keys_mutex = Rc::new(RefCell::new([false; 256]));
                    let mice_mutex = Rc::new(RefCell::new([0f32; 13]));

                    if debug {
                        loggy.send((
                            LogType::LuaSys,
                            "new controller connector starting".to_owned(),
                        ))?;
                    }
                    // gilrs has no Android backend, and `new()` reports that as
                    // `Err(NotImplemented)` — but that variant *carries a working
                    // instance* which simply lists no gamepads, which is exactly
                    // right for a phone. Unwrapping it aborted the process on
                    // startup (SIGABRT before the first frame), so take the
                    // instance and carry on without pads.
                    let mut gilrs = match Gilrs::new() {
                        Ok(g) => g,
                        Err(gilrs::Error::NotImplemented(g)) => {
                            loggy.send((
                                LogType::LuaSys,
                                "no gamepad support on this platform".to_owned(),
                            ))?;
                            g
                        }
                        // Anything else is a real failure with no instance to fall
                        // back on. Say which, rather than a bare unwrap panic.
                        Err(e) => {
                            loggy.send((
                                LogType::LuaSysError,
                                format!("gamepad subsystem failed to start: {}", e),
                            ))?;
                            panic!("gamepad subsystem failed to start: {}", e);
                        }
                    };
                    for (_id, gamepad) in gilrs.gamepads() {
                        loggy.send((
                            LogType::LuaSys,
                            format!("gamepad {} is {:?}", gamepad.name(), gamepad.power_info()),
                        ))?;
                    }

                    let pads = Rc::new(RefCell::new(Pad::new()));

                    let async_sender = pitcher.clone();

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
                    // Clone before handing the refs to the main thread: the loop needs
                    // them to publish each finished frame (see LuaImg::publish).
                    let gui_ref_local = main_ref
                        .upgrade()
                        .map(|w| WeakWrapper::from_wrapper(&w));
                    let sky_ref_local = sky_ref.upgrade().map(|w| WeakWrapper::from_wrapper(&w));
                    let pong = Box::new((main_ref, sky_ref));

                    async_sender.send((bundle_id, MainCommmand::InitBack(pong)))?;

                    match crate::command::init_lua_sys(
                        vm,
                        mc,
                        bundle_id,
                        privileged,
                        pitcher.clone(),
                        world_sender.clone(),
                        Rc::clone(&gui_handle),
                        #[cfg(feature = "audio")]
                        singer.clone(),
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
                    let main_fn =
                        vm.load_fn(mc, &mut compiler, Some("main_fn"), "main() loop()")?;
                    let loop_fn = vm.load_fn(mc, &mut compiler, Some("loop_fn"), "loop()")?;
                    let draw_fn = vm.load_fn(mc, &mut compiler, Some("draw_fn"), "draw()")?;
                    let drop_fn = vm.load_fn(mc, &mut compiler, Some("drop_fn"), "drop()")?;

                    // Persistent, non-'gc state the message handler needs. Built
                    // once here inside the single native enter; the wasm worker
                    // will build the same and re-enter per message (WASM.md §4a).
                    let mut ctx = LuaContext {
                        bundle_id,
                        compiler,
                        scripts,
                        loggy: loggy.clone(),
                        main_fn,
                        loop_fn,
                        draw_fn,
                        drop_fn,
                        keys_mutex,
                        diff_keys_mutex,
                        mice_mutex,
                        async_sender,
                        gui_ref: gui_ref_local,
                        sky_ref: sky_ref_local,
                    };

                    for m in &receiver {
                        // println!("{} {}", "[ 4 ]".on_bright_purple(), "lua loop recieve");

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
                        // Dispatch the message via the shared handler (also
                        // used by the wasm worker); Ok(true) means shut down.
                        if handle_lua_talk(m, vm, mc, &mut ctx, &mut local_pool, &shared)? {
                            break;
                        }
                    }
                    println!(
                        "{} {} {}",
                        "[ 5.9 ]".on_bright_purple(),
                        "fire the close signal",
                        bundle_id
                    );
                    ctx.async_sender
                        .send((ctx.bundle_id, MainCommmand::LuaClose()))?;
                    Ok(())
                })
            };

            let res = match thread_closure() {
                Ok(_) => Ok(()),
                Err(e) => Err(format!("lua ctx failure: {}", e)),
            };
            println!(
                "{} {} {}",
                "[ 6 ]".on_bright_purple(),
                "lua thread reached end for",
                bundle_id
            );
            res
        });

        thread_join
    }

    pub fn func(&self, func: &str) -> Result<LuaResponse, P64Error> {
        let (tx, rx) = sync_channel::<LuaResponse>(0);
        if let Some(ltx) = &self.to_lua_tx {
            ltx.send(LuaTalk::Func(func.to_string(), tx))
                .map_err(|_| P64Error::ChannelDisconnectedError)?;
            match rx.recv_timeout(Duration::from_millis(4000)) {
                Ok(lua_out) => Ok(lua_out),
                // Timeout: the thread is alive but never replied within the window
                // (stuck compiling/executing). Disconnected: the thread dropped the
                // reply sender — it died/panicked mid-call. These are very different
                // failures, so don't collapse them into one ambiguous timeout.
                Err(RecvTimeoutError::Timeout) => Err(P64Error::ChannelTimeoutError(1)),
                Err(RecvTimeoutError::Disconnected) => Err(P64Error::LuaClosed),
            }
        } else {
            Err(P64Error::LuaClosed)
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
        if let Some(ltx) = &self.to_lua_tx {
            ltx.send(LuaTalk::Load(Box::new(Script { name, content: buf }), tx))
                .map_err(|_| P64Error::ChannelDisconnectedError)?;

            rx.recv_timeout(Duration::from_millis(10000))
                .map_err(|e| match e {
                    RecvTimeoutError::Timeout => P64Error::ChannelTimeoutError(0),
                    RecvTimeoutError::Disconnected => P64Error::LuaClosed,
                })
        } else {
            Err(P64Error::LuaClosed)
        }
    }

    /** Call resize function with resolution within lua app */
    pub fn resize(&self, w: u32, h: u32) {
        if let Some(ltx) = &self.to_lua_tx {
            let _ = ltx.send(LuaTalk::Resize(w, h));
        }
    }

    pub fn async_load(
        &self,
        name: String,
        reader: &'lt mut (dyn Read + Send),
    ) -> Result<(), P64Error> {
        let mut buf = String::new();
        reader.read_to_string(&mut buf).unwrap(); // DEV can we get the reader instead?
        self.to_lua_tx
            .as_ref()
            .ok_or(P64Error::LuaClosed)?
            .send(LuaTalk::AsyncLoad(Box::new(Script { name, content: buf })))
            .map_err(|_| P64Error::ChannelDisconnectedError)?;
        Ok(())
    }

    /** Call main function within lua app */
    pub fn call_main(&self) -> Result<(), P64Error> {
        let ltx = self.to_lua_tx.as_ref().ok_or(P64Error::LuaClosed)?;
        ltx.send(LuaTalk::Main)
            .map_err(|_| P64Error::ChannelDisconnectedError)?;
        Ok(())
    }

    /** Call drop function within lua app */
    pub fn call_drop(&self, s: String) -> Result<(), P64Error> {
        if let Some(ltx) = &self.to_lua_tx {
            ltx.send(LuaTalk::Drop(s))
                .map_err(|_| P64Error::ChannelDisconnectedError)?;
        } else {
            println!("failed to call drop on shuttered lua instance");
        }
        Ok(())
    }

    /** Call loop function within lua app */
    pub fn call_loop(&self, bits: ControlState) -> Result<(), P64Error> {
        if let Some(ltx) = &self.to_lua_tx {
            if let Err(e) = ltx.send(LuaTalk::Loop(bits)) {
                println!("lua loop error: {}", e);
                return Err(P64Error::LuaLoopFail);
            }
        } else {
            println!("lua instance not available");
        }
        Ok(())
    }

    /** sends kill signal to this lua context thread */
    pub fn die(&self) -> Result<(), P64Error> {
        // let (tx, rx) =channel::<()>();
        let (tx, rx) = sync_channel::<()>(0);
        // self.async_inject(&"_self_destruct".to_string(), None);
        if let Some(ltx) = &self.to_lua_tx {
            let res = ltx.send(LuaTalk::Die(tx));
            if res.is_err() {
                Err(P64Error::LuaClosed)
            } else {
                match rx.recv_timeout(Duration::from_millis(5000)) {
                    Ok(_) => Ok(()),
                    // Disconnected: the thread is already gone — die() succeeded in
                    // spirit. Timeout: it's still alive but didn't ack the kill in 5s,
                    // i.e. wedged mid-call; report that distinctly.
                    Err(RecvTimeoutError::Disconnected) => Ok(()),
                    Err(RecvTimeoutError::Timeout) => Err(P64Error::ChannelTimeoutError(2)),
                }
            }
        } else {
            Err(P64Error::LuaClosed)
        }
    }

    pub fn close(&mut self) {
        self.to_lua_tx = None;
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
/// The Lua VM's persistent, non-`'gc` state — everything the message handler
/// needs that survives between `enter()` calls. On native it's built once inside
/// the single long-lived `enter` and borrowed by the message loop; on wasm it
/// lives outside the arena so each `postMessage` can re-`enter` and dispatch one
/// message (loaded fns persist in the VM via their `usize` indices, so nothing
/// here borrows `'gc`). See WASM.md §4a.
pub(crate) struct LuaContext {
    pub(crate) bundle_id: u8,
    pub(crate) compiler: Compiler,
    /// Sparse store of compiled sources, indexed by silt's source_index, read at
    /// error time to render a snippet against the originating source.
    pub(crate) scripts: Vec<Option<String>>,
    pub(crate) loggy: Sender<(LogType, String)>,
    /// External-function indices from load_fn; persist in the VM across enters.
    pub(crate) main_fn: usize,
    pub(crate) loop_fn: usize,
    pub(crate) draw_fn: usize,
    pub(crate) drop_fn: usize,
    pub(crate) keys_mutex: Rc<RefCell<[bool; 256]>>,
    pub(crate) diff_keys_mutex: Rc<RefCell<[bool; 256]>>,
    pub(crate) mice_mutex: Rc<RefCell<[f32; 13]>>,
    /// VM→host sink (the pitcher). On wasm this becomes a postMessage sink (§4b).
    pub(crate) async_sender: Sender<MainPacket>,
    /// Handles to this bundle's `gui` and `sky` rasters, used at the end of every
    /// loop to publish the finished frame for the renderer.
    pub(crate) gui_ref: Option<WeakWrapper>,
    pub(crate) sky_ref: Option<WeakWrapper>,
}

/// Publish a raster's finished frame, reporting whether it had anything new.
/// Runs on the owning Lua thread, so the copy can't race its own draw calls.
fn publish_raster(r: &Option<WeakWrapper>) -> bool {
    match r {
        Some(w) => match w.upgrade() {
            Some(mut ud) => ud
                .downcast_mut(|img: &mut crate::lua_img::LuaImg| Ok(img.publish()))
                .unwrap_or(false),
            None => false,
        },
        None => false,
    }
}

/// Dispatch a single `LuaTalk` message against the VM. Shared by the native
/// message loop and (next) the wasm worker. Returns `Ok(true)` when the runtime
/// should stop (a `Die` message). `local_pool`/`shared` are passed separately
/// because `LocalPool<'a>` borrows the `SharedPool` and so can't live inside the
/// non-borrowing `LuaContext`.
pub(crate) fn handle_lua_talk<'gc, 'a>(
    m: LuaTalk,
    vm: &mut VM<'gc>,
    mc: &Mutation<'gc>,
    ctx: &mut LuaContext,
    local_pool: &mut LocalPool<'a>,
    shared: &'a SharedPool,
) -> Result<bool, P64Error> {
    match m {
        LuaTalk::Load(code, sync) => {
            let Script { name, content } = *code;
            println!(
                "{} {} {} {}",
                "[ 3 ]".on_bright_purple(),
                "push first lua code payload id:",
                ctx.bundle_id,
                name
            );
            // run_in_context stores `content` into `scripts` at the index silt
            // assigns; nothing here needs to own a copy.
            let reply = match run_in_context(
                vm,
                mc,
                Some(&name),
                &content,
                &mut ctx.compiler,
                &mut ctx.scripts,
            ) {
                Err(er) => {
                    let s = error_string(er, &ctx.scripts);
                    ctx.loggy.send((LogType::LuaError, s.clone()))?;
                    LuaResponse::String(s)
                }
                Ok(v) => v,
            };
            // A failed reply only means the loader timed out and dropped its
            // receiver; that must NOT kill the runtime.
            if let Err(e) = sync.send(reply) {
                ctx.loggy.send((
                    LogType::LuaSysError,
                    format!("load reply dropped (caller gone): {}", e),
                ))?;
            }
        }
        LuaTalk::AsyncLoad(code) => {
            println!(
                "{} {} {} {}",
                "[ 3.5 ]".on_bright_purple(),
                "async push first lua code payload id:",
                ctx.bundle_id,
                code.name
            );
            if let Err(er) = run_in_context(
                vm,
                mc,
                Some(&code.name),
                &code.content,
                &mut ctx.compiler,
                &mut ctx.scripts,
            ) {
                let s = error_string(er, &ctx.scripts);
                ctx.loggy.send((LogType::LuaError, s))?;
            }
        }
        LuaTalk::Main => {
            if let Err(er) = vm.call_fn(mc, Some("main_fn_call"), ctx.main_fn, ()) {
                // Runtime error inside main() points into the loaded app source.
                let s = error_string(er.into(), &ctx.scripts);
                ctx.loggy.send((LogType::LuaError, s))?;
            };
        }
        LuaTalk::Die(sync) => {
            println!(
                "{} {}",
                "[ 5 ]".on_bright_purple(),
                "we got permission to die :)"
            );
            sync.send(())?;
            return Ok(true);
        }
        LuaTalk::AsyncFunc(_func) => {}
        LuaTalk::Loop(control_state) => {
            let ControlState(key_state, mouse_state) = control_state;

            // Refresh input state BEFORE loop() runs so keys()/mus() inside the
            // app read THIS frame's input. Updating it after the call (as it was)
            // meant every frame saw the previous frame's input — a one-frame lag.
            let mut h = ctx.diff_keys_mutex.borrow_mut();
            ctx.keys_mutex.borrow().iter().enumerate().for_each(|(i, k)| {
                h[i] = !k && key_state[i];
            });
            drop(h);

            *ctx.keys_mutex.borrow_mut() = key_state;
            // Carry the prior frame's x,y into px,py before overwriting them.
            // The `[...]` is fully evaluated before the assignment lands, so
            // `mm[0]`/`mm[1]` on the right still read last frame's position.
            let mut mm = ctx.mice_mutex.borrow_mut();
            *mm = [
                mouse_state[0],
                mouse_state[1],
                mouse_state[2],
                mouse_state[3],
                mm[0],
                mm[1],
                mouse_state[4],
                mouse_state[5],
                mouse_state[6],
                mouse_state[7],
                mouse_state[8],
                mouse_state[9],
                mouse_state[10],
            ];
            drop(mm);

            if let Err(e) = vm.call_fn(mc, Some("loop_fn_call"), ctx.loop_fn, ()) {
                // Runtime error inside loop() points into the loaded app source.
                let s = error_string(e.into(), &ctx.scripts);
                ctx.loggy.send((LogType::LuaError, s))?;
            };

            local_pool.check_lock(shared);

            // Publish whatever this loop drew, and report only what actually
            // changed. This used to hand back a hardcoded `true` for both, so a
            // still screen re-uploaded the same megabyte every frame per layer.
            let mutations = BundleMutations {
                gui: publish_raster(&ctx.gui_ref),
                sky: publish_raster(&ctx.sky_ref),
            };
            ctx.async_sender
                .send((ctx.bundle_id, MainCommmand::LoopComplete(mutations)))?;
            local_pool.drop();
        }
        LuaTalk::Func(func, sync) => {
            // A Lua error here is the caller's problem, not a reason to tear down
            // the whole runtime: report it back and keep looping.
            let res = match run_in_context(
                vm,
                mc,
                Some("func ->"),
                &func,
                &mut ctx.compiler,
                &mut ctx.scripts,
            ) {
                Ok(v) => v,
                Err(e) => {
                    let s = error_string(e, &ctx.scripts);
                    ctx.loggy.send((LogType::LuaError, s.clone()))?;
                    LuaResponse::String(s)
                }
            };
            sync.send(res)?
        }
        LuaTalk::Resize(w, h) => {
            println!("resize {} {}", w, h);
            // A throwing draw() must not kill the runtime; log and continue.
            if let Err(e) = vm.call_fn(mc, Some("redraw_fn"), ctx.draw_fn, (w, h)) {
                let s = error_string(e.into(), &ctx.scripts);
                ctx.loggy.send((LogType::LuaError, s))?;
            }
        }
        LuaTalk::Drop(s) => {
            let res = vm.call_fn(mc, Some("drop"), ctx.drop_fn, s);
            if let Err(e) = res {
                let msg = error_string(e.into(), &ctx.scripts);
                ctx.async_sender
                    .send((ctx.bundle_id, MainCommmand::AsyncError(msg)))?;
            }
        }
    }
    Ok(false)
}

fn run_in_context<'gc>(
    vm: &mut VM<'gc>,
    mc: &Mutation<'gc>,
    name: Option<&str>,
    // Borrow the source, never copy it here. It's stored (one owned copy) into
    // `scripts` at the index silt assigns, so `snippet` can find it at error time.
    code: &str,
    compiler: &mut Compiler,
    scripts: &mut Vec<Option<String>>,
) -> Result<ExVal, P64Error> {
    // NOTE: use the shared `compiler` passed in (as the original build_and_run
    // did). A fresh Compiler::new() per call was tried as a workaround but appears
    // to wedge when compiling a call expression like `help(true)`; the shared one
    // is the known-good path now that the silt-side bug is fixed.
    let compiled = match compiler.try_compile(mc, name, code) {
        Ok(f) => f,
        Err(e) => {
            // Record the source at the failed index too, so the compile-error
            // snippet resolves against it just like a runtime error would.
            store_script(scripts, e.source_index, code);
            return Err(e.into());
        }
    };
    // Stamp the compiled source into the store at its assigned index so any later
    // runtime error carrying this index (including from nested functions defined
    // here) can be rendered as a snippet.
    store_script(scripts, compiled.source_index, code);
    vm.run(mc, silt_lua::gc_arena::Gc::new(mc, compiled))
        .map_err(|e| e.into())
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
    re.map_err(|e| P64Error::from(e))
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

//! The browser's audio output, with the low-latency AudioWorklet path and the
//! main-thread scheduler as a fallback.
//!
//! Two ways to get sound out of a browser:
//!
//! - **AudioWorklet** (preferred): the synth runs *inside* the browser's audio
//!   rendering thread, filling 128-frame blocks (~2.7 ms at 48 kHz). Latency is
//!   just the browser's output floor. Setup is asynchronous (a module script must
//!   load and the wasm must be compiled and handed over), so it can't be finished
//!   inside a synchronous constructor.
//! - **`WebAudioOut`** (fallback, in `sound.rs`): generates audio on the *main*
//!   thread and schedules it ahead on the AudioContext clock. Glitch-free through
//!   frame jank, but it needs ~90 ms of lookahead to be so — fine for playback,
//!   too laggy to play music with.
//!
//! [`WebOut`] hides that: it's built synchronously, immediately hands the engine a
//! `Sender<SoundCommand>`, and loads the worklet in the background. Commands sent
//! during setup are held and flushed once the node exists, so nothing is lost at
//! boot. If setup fails, it degrades to the scheduler rather than going silent.
//!
//! **Startup order matters more than it looks.** An `AudioWorkletProcessor` is
//! constructed on the audio *rendering* thread, and a suspended AudioContext never
//! starts that thread — browsers keep it suspended until the page has a user
//! gesture. Creating the node and posting it the wasm module while suspended left
//! the processor unconstructed, so it never received the module, never reported
//! back, and never produced a sample: silence with a completely clean log. So the
//! module is loaded and compiled eagerly (that part is fine while suspended) but
//! the node is not created until [`WebOut::pump`] sees the context actually running.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};

use crate::sound::{SoundCommand, WebAudioOut};

/// Where the worklet's module script and the synth's wasm are served from. Both
/// are copied into the dist root by Trunk (see `web/index.html`); the glue JS is
/// imported *by the worklet*, which is why it must sit beside it.
const WORKLET_JS: &str = "synth-worklet.js";
const SYNTH_WASM: &str = "petrichor_synth_bg.wasm";
/// Registered name in `web/synth-worklet.js`.
const PROCESSOR: &str = "petrichor-synth";

/// How far worklet setup has got. Shared between the async loader and `pump`.
enum Stage {
    /// Loading the processor module and compiling the synth's wasm.
    Loading,
    /// Both are ready; waiting for the context to run before creating the node
    /// (see the module docs — a node created while suspended may never come alive).
    Compiled(JsValue),
    /// The worklet is live — commands go straight to its port.
    Worklet(web_sys::AudioWorkletNode),
    /// Setup failed; the main-thread scheduler took over.
    Fallback,
}

/// The browser audio output.
pub struct WebOut {
    ctx: web_sys::AudioContext,
    stage: Rc<RefCell<Stage>>,
    /// Commands from the engine, drained by `pump`.
    audience: Receiver<SoundCommand>,
    /// Buffered until there's somewhere to send them.
    pending: Vec<SoundCommand>,
    /// Built only if we end up on the fallback path, along with the sender that
    /// feeds its mixer the commands the engine is already producing.
    fallback: Option<WebAudioOut>,
    fallback_tx: Option<Sender<SoundCommand>>,
    /// Set if constructing the fallback failed, so we don't retry every frame.
    fallback_failed: bool,
    /// Frames until the next resume() attempt (see pump).
    resume_wait: u32,
    /// Logged once, so a suspended context doesn't spam every frame.
    warned_suspended: bool,
    /// Set when the worklet reports its synth is live. Watched so a worklet that
    /// starts but never becomes usable falls back instead of playing silence.
    ready: Rc<std::cell::Cell<bool>>,
    /// Frames since the node was created, for that timeout.
    worklet_frames: u32,
}

impl WebOut {
    /// Build the output and start (but don't await) loading the worklet.
    pub fn new(audience: Receiver<SoundCommand>) -> Result<Self, JsValue> {
        let ctx = web_sys::AudioContext::new()?;
        let stage = Rc::new(RefCell::new(Stage::Loading));
        log::info!(
            "web audio: {} Hz, loading AudioWorklet ({})",
            ctx.sample_rate(),
            PROCESSOR
        );
        spawn_local(load(ctx.clone(), stage.clone()));
        Ok(Self {
            ctx,
            stage,
            audience,
            pending: Vec::new(),
            fallback: None,
            fallback_tx: None,
            fallback_failed: false,
            resume_wait: 1,
            warned_suspended: false,
            ready: Rc::new(std::cell::Cell::new(false)),
            worklet_frames: 0,
        })
    }

    /// A clone of the AudioContext (a JS reference) so a gesture handler can
    /// resume it — browsers start it suspended until the first user interaction.
    pub fn context(&self) -> web_sys::AudioContext {
        self.ctx.clone()
    }

    /// Move audio forward. Call once per frame.
    ///
    /// On the worklet path this only *forwards commands* — the audio itself is
    /// generated on the audio thread, so a slow frame no longer costs us samples.
    /// On the fallback path it also runs the chunk scheduler.
    pub fn pump(&mut self) {
        let running = self.ctx.state() == web_sys::AudioContextState::Running;
        if !running {
            // Belt and braces alongside the engine's gesture handlers. Retried
            // sparsely, not every frame: before the page has user activation the
            // browser both rejects this *and* logs a warning, so per-frame retries
            // bury the console in "AudioContext was not allowed to start".
            self.resume_wait = self.resume_wait.saturating_sub(1);
            if self.resume_wait == 0 {
                self.resume_wait = 60;
                let _ = self.ctx.resume();
            }
            if !self.warned_suspended {
                self.warned_suspended = true;
                log::info!("web audio: context suspended, waiting for a user gesture");
            }
        }

        // Create the worklet node the moment the context is genuinely running —
        // not before. The processor is constructed on the audio rendering thread,
        // which a suspended context never starts.
        if running {
            let ready_module = match &*self.stage.borrow() {
                Stage::Compiled(m) => Some(m.clone()),
                _ => None,
            };
            if let Some(module) = ready_module {
                match self.start_worklet(&module) {
                    Ok(node) => {
                        log::info!("web audio: AudioWorklet started (low latency path)");
                        *self.stage.borrow_mut() = Stage::Worklet(node);
                    }
                    Err(e) => {
                        log::error!(
                            "web audio: could not start the worklet node ({:?}); \
                             falling back to the main-thread scheduler",
                            e
                        );
                        *self.stage.borrow_mut() = Stage::Fallback;
                    }
                }
            }
        }

        // Collect whatever the engine has queued since the last frame.
        for cmd in self.audience.try_iter() {
            self.pending.push(cmd);
        }

        let stage_is = match &*self.stage.borrow() {
            Stage::Loading | Stage::Compiled(_) => 0,
            Stage::Worklet(_) => 1,
            Stage::Fallback => 2,
        };

        match stage_is {
            // Still starting up: hold commands (bounded, so a stalled setup can't
            // grow without limit — oldest go first, as with any dropped queue).
            0 => {
                const MAX_PENDING: usize = 4096;
                if self.pending.len() > MAX_PENDING {
                    let excess = self.pending.len() - MAX_PENDING;
                    self.pending.drain(..excess);
                }
            }
            1 => {
                // A worklet that started but never reported a live synth would
                // play silence forever. Give the handshake a few seconds, then
                // take the scheduler instead — degraded beats mute.
                if !self.ready.get() {
                    self.worklet_frames += 1;
                    if self.worklet_frames > 240 {
                        log::error!(
                            "web audio: worklet never reported a ready synth; \
                             falling back to the main-thread scheduler"
                        );
                        *self.stage.borrow_mut() = Stage::Fallback;
                        return;
                    }
                }
                if !self.pending.is_empty() {
                    if let Stage::Worklet(node) = &*self.stage.borrow() {
                        if let Ok(port) = node.port() {
                            for cmd in self.pending.drain(..) {
                                post_command(&port, &cmd);
                            }
                        }
                    }
                }
            }
            _ => {
                // Fallback: build the scheduler on first use, then feed it the
                // same command stream and let it generate audio here.
                if self.fallback.is_none() && !self.fallback_failed {
                    let (tx, rx) = channel::<SoundCommand>();
                    // Share our AudioContext: it's the one the engine's gesture
                    // handler resumes, and a suspended context stays silent.
                    match WebAudioOut::with_context(self.ctx.clone(), rx) {
                        Ok(out) => {
                            self.fallback = Some(out);
                            self.fallback_tx = Some(tx);
                        }
                        Err(e) => {
                            log::error!("web audio fallback failed: {:?}", e);
                            self.fallback_failed = true;
                        }
                    }
                }
                if let Some(tx) = &self.fallback_tx {
                    for cmd in self.pending.drain(..) {
                        let _ = tx.send(cmd);
                    }
                } else {
                    self.pending.clear(); // nowhere to send; don't grow forever
                }
                if let Some(out) = self.fallback.as_mut() {
                    out.pump();
                }
            }
        }
    }

    /// Create the node, wire it to the speakers, listen for its reports, and hand
    /// it the compiled module. Called from `pump` once the context is running.
    fn start_worklet(&self, module: &JsValue) -> Result<web_sys::AudioWorkletNode, JsValue> {
        let node = web_sys::AudioWorkletNode::new(&self.ctx, PROCESSOR)?;
        node.connect_with_audio_node(&self.ctx.destination())?;
        let port = node.port()?;

        // Listen before handing over the module. The processor reports its own
        // failures (e.g. the wasm not instantiating inside the worklet) — without
        // this, such a failure is a silent page with no fallback and no message.
        let watched = self.stage.clone();
        let ready_flag = self.ready.clone();
        // Captured so the handshake below can hand the module over.
        let pending_module = module.clone();
        let handshake_port = port.clone();
        let rate = self.ctx.sample_rate() as f64;
        let on_msg =
            Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |e: web_sys::MessageEvent| {
                let data = e.data();
                let field = |k: &str| {
                    js_sys::Reflect::get(&data, &k.into())
                        .ok()
                        .and_then(|v| v.as_string())
                        .unwrap_or_default()
                };
                let num = |k: &str| {
                    js_sys::Reflect::get(&data, &k.into())
                        .ok()
                        .and_then(|v| v.as_f64())
                        .unwrap_or(f64::NAN)
                };
                match field("type").as_str() {
                    // Posted from the processor's constructor. This is the
                    // handshake: only now does a processor exist to receive the
                    // module. Posting it at node-creation time raced construction
                    // on the audio thread and the message was silently dropped —
                    // the processor came up and then sat there with no synth.
                    "hello" => {
                        log::info!("web audio: worklet processor constructed, sending synth wasm");
                        let msg = js_sys::Object::new();
                        let _ = js_sys::Reflect::set(&msg, &"type".into(), &"wasm".into());
                        let _ = js_sys::Reflect::set(&msg, &"module".into(), &pending_module);
                        // Pass the rate we measured rather than trusting the
                        // worklet scope's global: a non-finite rate there would make
                        // every phase increment NaN — silence with no error at all.
                        let _ = js_sys::Reflect::set(
                            &msg,
                            &"sampleRate".into(),
                            &JsValue::from_f64(rate),
                        );
                        let _ = handshake_port.post_message(&msg);
                    }
                    "ready" => {
                        ready_flag.set(true);
                        log::info!("web audio: AudioWorklet synth ready @ {} Hz", num("rate"))
                    }
                    // Says which link is broken when there's no sound and no error:
                    // cmds 0 => commands aren't arriving; peak 0 => the mixer is
                    // producing silence.
                    "stats" => log::info!(
                        "web audio: blocks={} cmds={} peak={:.4} channels={} frames={}",
                        num("blocks"),
                        num("cmds"),
                        num("peak"),
                        num("channels"),
                        num("frames"),
                    ),
                    "error" => {
                        log::error!(
                            "web audio: worklet failed ({}); falling back to the \
                             main-thread scheduler (higher latency)",
                            field("message")
                        );
                        *watched.borrow_mut() = Stage::Fallback;
                    }
                    _ => {}
                }
            });
        port.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
        // Setting onmessage also starts the port, so anything the processor already
        // queued arrives.
        on_msg.forget();

        // The module is deliberately NOT posted here — see the "hello" arm above.
        Ok(node)
    }
}

/// Serialize one command as MessagePack and post it to the worklet. A binary
/// codec matters here: a command can carry a whole decoded ogg as `Vec<f32>`,
/// which as a JS array would be one boxed number per sample.
fn post_command(port: &web_sys::MessagePort, cmd: &SoundCommand) {
    let bytes = match rmp_serde::to_vec(cmd) {
        Ok(b) => b,
        Err(e) => {
            log::error!("sound command encode failed: {}", e);
            return;
        }
    };
    let msg = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&msg, &"type".into(), &"cmd".into());
    let _ = js_sys::Reflect::set(
        &msg,
        &"bytes".into(),
        &js_sys::Uint8Array::from(&bytes[..]).into(),
    );
    let _ = port.post_message(&msg);
}

/// Register the processor module and compile the synth's wasm. Both are safe to do
/// while the context is suspended; creating the *node* is not, so that waits for
/// `pump`.
async fn load(ctx: web_sys::AudioContext, stage: Rc<RefCell<Stage>>) {
    match try_load(&ctx).await {
        Ok(module) => {
            log::info!("web audio: worklet module + synth wasm loaded");
            *stage.borrow_mut() = Stage::Compiled(module);
        }
        Err(e) => {
            log::warn!(
                "web audio: AudioWorklet unavailable ({:?}); falling back to the \
                 main-thread scheduler (higher latency)",
                e
            );
            *stage.borrow_mut() = Stage::Fallback;
        }
    }
}

/// `AudioWorkletGlobalScope` has no `fetch`, so the worklet cannot load its own
/// wasm — the main thread compiles it here and posts the `WebAssembly.Module`
/// over the port. Compiled modules are structured-cloneable and browsers share the
/// compiled code across instances, so this costs no extra download and duplicates
/// no code, only the instance's linear memory.
async fn try_load(ctx: &web_sys::AudioContext) -> Result<JsValue, JsValue> {
    let worklet = ctx.audio_worklet()?;
    JsFuture::from(worklet.add_module(WORKLET_JS)?).await?;

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let resp = JsFuture::from(window.fetch_with_str(SYNTH_WASM)).await?;
    let resp: web_sys::Response = resp.dyn_into()?;
    let buf = JsFuture::from(resp.array_buffer()?).await?;
    JsFuture::from(js_sys::WebAssembly::compile(&buf.into())).await
}

/// Create the browser audio driver + the command sender the engine sends notes on.
pub fn init_web() -> (Result<WebOut, JsValue>, Sender<SoundCommand>) {
    let (singer, audience) = channel::<SoundCommand>();
    (WebOut::new(audience), singer)
}

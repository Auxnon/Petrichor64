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
//! `Sender<SoundCommand>`, and kicks off worklet setup in the background. Commands
//! sent during setup are held and flushed once the node exists, so nothing is lost
//! at boot. If setup fails, it degrades to the scheduler rather than going silent.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::{channel, Receiver, Sender};

use wasm_bindgen::prelude::*;
use wasm_bindgen::closure::Closure;
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

/// How far worklet setup has got. Shared between the async setup task and `pump`.
enum Stage {
    /// Still loading; commands wait in `pending`.
    Setup,
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
    /// Buffered while the worklet is still starting up.
    pending: Vec<SoundCommand>,
    /// Built only if we end up on the fallback path, along with the sender that
    /// feeds its mixer the commands the engine is already producing.
    fallback: Option<WebAudioOut>,
    fallback_tx: Option<Sender<SoundCommand>>,
    /// Logged once, so a suspended context doesn't spam every frame.
    warned_suspended: bool,
    /// Set if constructing the fallback failed, so we don't retry every frame.
    fallback_failed: bool,
}

impl WebOut {
    /// Build the output and start (but don't await) worklet setup.
    pub fn new(audience: Receiver<SoundCommand>) -> Result<Self, JsValue> {
        let ctx = web_sys::AudioContext::new()?;
        let stage = Rc::new(RefCell::new(Stage::Setup));
        log::info!(
            "web audio: {} Hz, starting AudioWorklet ({})",
            ctx.sample_rate(),
            PROCESSOR
        );
        spawn_local(setup(ctx.clone(), stage.clone()));
        Ok(Self {
            ctx,
            stage,
            audience,
            pending: Vec::new(),
            fallback: None,
            fallback_tx: None,
            fallback_failed: false,
            warned_suspended: false,
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
        // Nothing can be delivered before the first user gesture resumes the
        // context; hold commands rather than dropping them.
        let running = self.ctx.state() == web_sys::AudioContextState::Running;
        // Belt and braces alongside the engine's gesture handlers: retry resume
        // here too. Browsers only honour resume() once the page has had *some*
        // user activation, and this runs every frame, so the first frame after any
        // interaction gets it — no reliance on one handler being wired up.
        if !running {
            let _ = self.ctx.resume();
            if !self.warned_suspended {
                self.warned_suspended = true;
                log::info!("web audio: context suspended, waiting for a user gesture");
            }
        }

        let stage_is = match &*self.stage.borrow() {
            Stage::Setup => 0,
            Stage::Worklet(_) => 1,
            Stage::Fallback => 2,
        };

        // Collect whatever the engine has queued since the last frame.
        for cmd in self.audience.try_iter() {
            self.pending.push(cmd);
        }

        match stage_is {
            // Still starting up: keep holding (bounded, so a stalled setup can't
            // grow without limit — the oldest go first, as with a dropped queue).
            0 => {
                const MAX_PENDING: usize = 4096;
                if self.pending.len() > MAX_PENDING {
                    let excess = self.pending.len() - MAX_PENDING;
                    self.pending.drain(..excess);
                }
            }
            1 => {
                if running && !self.pending.is_empty() {
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

/// Load the worklet module, compile the synth's wasm, create the node and hand the
/// compiled module over. `AudioWorkletGlobalScope` has no `fetch`, so compiling
/// here on the main thread and posting the `WebAssembly.Module` is the only way in
/// — and since compiled modules are structured-cloneable and browsers share the
/// compiled code, it costs no extra download and duplicates no code.
async fn setup(ctx: web_sys::AudioContext, stage: Rc<RefCell<Stage>>) {
    match try_setup(&ctx).await {
        Ok(node) => {
            // `addModule` rejecting covers a worklet that fails to *load*, but not
            // one that fails once running (say the wasm not instantiating inside
            // it). The processor reports that over the port; without listening,
            // such a failure is a silent page with no fallback and no explanation.
            if let Ok(port) = node.port() {
                let watched = stage.clone();
                let on_msg = Closure::<dyn FnMut(web_sys::MessageEvent)>::new(
                    move |e: web_sys::MessageEvent| {
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
                            "ready" => {
                                log::info!("web audio: AudioWorklet synth ready @ {} Hz", num("rate"))
                            }
                            // Says which link is broken when there's no sound and
                            // no error: no stats at all => we're not being
                            // rendered; cmds 0 => commands aren't arriving;
                            // peak 0 => the mixer is producing silence.
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
                    },
                );
                port.set_onmessage(Some(on_msg.as_ref().unchecked_ref()));
                // Setting onmessage also starts the port, so anything the worklet
                // already queued (it may have replied before we got here) arrives.
                on_msg.forget();
            }
            log::info!("web audio: AudioWorklet running (low latency path)");
            *stage.borrow_mut() = Stage::Worklet(node);
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

async fn try_setup(ctx: &web_sys::AudioContext) -> Result<web_sys::AudioWorkletNode, JsValue> {
    // 1. Register the processor module.
    let worklet = ctx.audio_worklet()?;
    JsFuture::from(worklet.add_module(WORKLET_JS)?).await?;

    // 2. Fetch + compile the synth wasm on this thread.
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let resp = JsFuture::from(window.fetch_with_str(SYNTH_WASM)).await?;
    let resp: web_sys::Response = resp.dyn_into()?;
    let buf = JsFuture::from(resp.array_buffer()?).await?;
    let module = JsFuture::from(js_sys::WebAssembly::compile(&buf.into())).await?;

    // 3. Create the node, wire it to the speakers, and hand it the module.
    let node = web_sys::AudioWorkletNode::new(ctx, PROCESSOR)?;
    node.connect_with_audio_node(&ctx.destination())?;
    let port = node.port()?;
    let msg = js_sys::Object::new();
    js_sys::Reflect::set(&msg, &"type".into(), &"wasm".into())?;
    js_sys::Reflect::set(&msg, &"module".into(), &module)?;
    // Pass the rate we measured rather than relying on the worklet scope's global:
    // a non-finite rate in there would make every phase increment NaN, i.e. silence
    // with no error anywhere.
    js_sys::Reflect::set(
        &msg,
        &"sampleRate".into(),
        &JsValue::from_f64(ctx.sample_rate() as f64),
    )?;
    port.post_message(&msg)?;
    Ok(node)
}

/// Create the browser audio driver + the command sender the engine sends notes on.
pub fn init_web() -> (Result<WebOut, JsValue>, Sender<SoundCommand>) {
    let (singer, audience) = channel::<SoundCommand>();
    (WebOut::new(audience), singer)
}

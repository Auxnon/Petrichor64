#[cfg(unix)]
mod capture;
mod input;
mod renderer;

use crate::log::LogType;
use crate::root::Core;
use input::TuiInput;
use pollster::block_on;
use renderer::TuiRenderer;
use std::io::stdout;
use std::time::{Duration, Instant};

use crossterm::{
    cursor::{Hide, Show},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

/// Entry point for the software-rasterized terminal renderer. Structurally
/// mirrors the headless `start()` loop in lib.rs (same `Core::new()`, same
/// Lua/world/ent_manager tick) but adds a per-tick keyboard poll + render on
/// top, so terrain/entities driven by the same Lua game logic other backends
/// run are also drawn — see the plan at inherited-prancing-dahl.md for why
/// this doesn't share code with the wgpu `render_loop`.
pub fn start() {
    env_logger::init();
    let (mut core, catcher) = block_on(Core::new());

    crate::command::load_empty(&mut core);
    crate::command::hard_reset(&mut core);
    // See `crate::drain_stray_messages`: the boot logo's Lua thread can still
    // be mid-flight on its `attr{fog=200}` write when hard_reset tears it
    // down (`Lua::die` only waits up to 250ms, a grace period not a
    // guarantee); left in the queue it lands on whatever bundle id gets
    // reused next instead — the very app about to load below.
    crate::drain_stray_messages(&catcher);
    // Same "CLI arg, else auto-load" precedence the headed/headless `start()`
    // paths use (`lib.rs`) — this used to hardcode "test/basic", silently
    // ignoring whatever game path was actually passed on the command line.
    let game_path = if std::env::args().count() > 1 {
        Some(std::env::args().nth(1).unwrap())
    } else {
        crate::asset::check_for_auto()
    };
    if let Err(e) = crate::command::load_app(&mut core, game_path.as_deref(), None, None, None) {
        core.loggy.log(LogType::CoreError, &format!("{}", e));
    }

    // From here on the engine's printing goes to the log strip, not over the
    // picture (see capture.rs); frames are drawn on `term`.
    #[cfg(unix)]
    let capture = capture::Capture::start();
    #[cfg(unix)]
    if let Some(c) = &capture {
        capture::restore_on_panic(c.saved_err());
    }
    let mut term: Box<dyn std::io::Write> = Box::new(stdout());
    #[cfg(unix)]
    if let Some(Ok(f)) = capture.as_ref().map(|c| c.term.try_clone()) {
        term = Box::new(f);
    }

    let raw_mode_enabled = enable_raw_mode().is_ok();
    let _ = execute!(term, EnterAlternateScreen, Hide);

    let mut tui_input = TuiInput::new();
    let mut renderer = TuiRenderer::new();
    let frame = Duration::from_secs_f32(1.0 / crate::FPS);

    loop {
        let start = Instant::now();
        core.loop_helper.loop_start();

        while let Ok(line) = core.cli_thread_receiver.try_recv() {
            crate::core_console_command(&mut core, line.trim(), Some(&catcher));
        }

        // Drain queued log messages (Lua runtime errors included) and print
        // them. The headed backend only does this inside its gui console
        // overlay (`Gui::render`, gated `#[cfg(feature = "headed")]`), so the
        // TUI backend never saw them at all: a throwing loop() would render
        // nothing and log nothing, silently.
        core.loggy.listen();

        if tui_input.poll() {
            break;
        }

        core.update(&catcher);
        core.bundle_manager
            .call_loop(&mut core.completed_bundles, tui_input.state());

        if let Some(fps) = core.loop_helper.report_rate() {
            core.global.fps = fps;
        }
        #[cfg(unix)]
        let log = capture.as_ref().map(|c| c.tail.recent(4)).unwrap_or_default();
        #[cfg(not(unix))]
        let log: Vec<String> = Vec::new();
        renderer.render_frame(&mut core, &mut term, &log);

        if let Some(rem) = frame.checked_sub(start.elapsed()) {
            std::thread::sleep(rem);
        }
    }

    let _ = execute!(term, Show, LeaveAlternateScreen);
    if raw_mode_enabled {
        let _ = disable_raw_mode();
    }
    #[cfg(unix)]
    if let Some(c) = capture {
        c.finish();
    }
}

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
    if let Err(e) = crate::command::load_app(&mut core, Some("test/basic"), None, None, None) {
        core.loggy.log(LogType::CoreError, &format!("{}", e));
    }

    let mut out = stdout();
    let raw_mode_enabled = enable_raw_mode().is_ok();
    let _ = execute!(out, EnterAlternateScreen, Hide);

    let mut tui_input = TuiInput::new();
    let mut renderer = TuiRenderer::new();
    let frame = Duration::from_secs_f32(1.0 / crate::FPS);

    loop {
        let start = Instant::now();

        while let Ok(line) = core.cli_thread_receiver.try_recv() {
            crate::core_console_command(&mut core, line.trim());
        }

        if tui_input.poll() {
            break;
        }

        core.update(&catcher);
        core.bundle_manager
            .call_loop(&mut core.completed_bundles, tui_input.state());

        if let Some(fps) = core.loop_helper.report_rate() {
            core.global.fps = fps;
        }
        renderer.render_frame(&mut core);

        if let Some(rem) = frame.checked_sub(start.elapsed()) {
            std::thread::sleep(rem);
        }
    }

    let _ = execute!(out, Show, LeaveAlternateScreen);
    if raw_mode_enabled {
        let _ = disable_raw_mode();
    }
}

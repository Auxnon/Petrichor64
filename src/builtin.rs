//! Apps compiled into the engine. Only overlays live here today: they get the
//! privileged `app.*` table, so their code must come from the engine itself,
//! never from the app being edited (PLAN.md, overlay trust boundary). The
//! sources stay under `apps/` so they can also be run from disk while working
//! on them (`overlay apps/paint`).

/// The scripts of a built-in app, keyed by the directory name it loads under.
pub fn scripts(directory: &str) -> Option<&'static [(&'static str, &'static str)]> {
    match directory {
        crate::command::PAINT_OVERLAY => Some(PAINT),
        _ => None,
    }
}

/// Ctrl+P: pick a mesh, sprite or tile in the running app and paint its
/// texture in place.
static PAINT: &[(&str, &str)] = &[
    ("canvas.lua", include_str!("../apps/paint3d/scripts/canvas.lua")),
    ("main.lua", include_str!("../apps/paint3d/scripts/main.lua")),
    ("paint.lua", include_str!("../apps/paint3d/scripts/paint.lua")),
    ("pal.lua", include_str!("../apps/paint3d/scripts/pal.lua")),
    ("panel.lua", include_str!("../apps/paint3d/scripts/panel.lua")),
    ("save.lua", include_str!("../apps/paint3d/scripts/save.lua")),
    ("scene.lua", include_str!("../apps/paint3d/scripts/scene.lua")),
];

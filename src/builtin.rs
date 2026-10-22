//! Apps compiled into the engine. Only overlays live here today: they get the
//! privileged `app.*` table, so their code must come from the engine itself,
//! never from the app being edited (PLAN.md, overlay trust boundary).
//!
//! The image editor lives in its own repo, github.com/Auxnon/image-chor. With
//! the `studio` feature, build.rs fetches its latest `main` (or the checkout in
//! $IMAGE_CHOR_DIR) and generates the script list included below. Run it from
//! disk while working on it: `chor game --overlay ~/projects/image-chor`.

mod image_chor {
    include!(concat!(env!("OUT_DIR"), "/image_chor.rs"));
}

/// The image editor's commit, or "" when it isn't built in.
pub use image_chor::PAINT_REV;

/// The scripts of a built-in app, keyed by the directory name it loads under.
/// None for the image editor when it isn't built in (no `studio`, or the
/// build couldn't fetch it).
pub fn scripts(directory: &str) -> Option<&'static [(&'static str, &'static str)]> {
    match directory {
        crate::command::PAINT_OVERLAY if !image_chor::PAINT.is_empty() => Some(image_chor::PAINT),
        _ => None,
    }
}

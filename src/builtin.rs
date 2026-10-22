//! Apps compiled into the engine. Only overlays live here today: they get the
//! privileged `app.*` table, so their code must come from the engine itself,
//! never from the app being edited (PLAN.md, overlay trust boundary).
//!
//! The editors live in their own repos: the image editor (Ctrl+P) in
//! github.com/Auxnon/image-chor, the model editor (Ctrl+M) in
//! github.com/Auxnon/model-chor. With the `studio` feature, build.rs fetches
//! each one's latest `main` (or the checkout in $IMAGE_CHOR_DIR /
//! $MODEL_CHOR_DIR) and generates the script lists included below. Run one
//! from disk while working on it: `chor game --overlay ~/projects/model-chor`.

mod image_chor {
    include!(concat!(env!("OUT_DIR"), "/image_chor.rs"));
}

mod model_chor {
    include!(concat!(env!("OUT_DIR"), "/model_chor.rs"));
}

/// The image editor's commit, or "" when it isn't built in.
pub use image_chor::PAINT_REV;
/// The model editor's commit, or "" when it isn't built in.
pub use model_chor::MODEL_REV;

/// The scripts of a built-in app, keyed by the directory name it loads under.
/// None for an editor that isn't built in (no `studio`, or the build couldn't
/// fetch it).
pub fn scripts(directory: &str) -> Option<&'static [(&'static str, &'static str)]> {
    match directory {
        crate::command::PAINT_OVERLAY if !image_chor::PAINT.is_empty() => Some(image_chor::PAINT),
        crate::command::MODEL_OVERLAY if !model_chor::MODEL.is_empty() => Some(model_chor::MODEL),
        _ => None,
    }
}

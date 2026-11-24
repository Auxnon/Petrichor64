//! Apps compiled into the engine. Only overlays live here today: they get the
//! privileged `app.*` table, so their code must come from the engine itself,
//! never from the app being edited (PLAN.md, overlay trust boundary).
//!
//! The tools live in their own repos, all under github.com/Auxnon:
//!
//! - image-chor, the image editor (Ctrl+P)
//! - model-chor, the model editor (Ctrl+M)
//! - edit-chor, the source editor (Ctrl+E)
//! - sound-chor, the sound tool (Ctrl+N)
//!
//! With the `studio` feature, build.rs fetches each one's latest `main` (or
//! the checkout in $IMAGE_CHOR_DIR, $MODEL_CHOR_DIR, $EDIT_CHOR_DIR,
//! $SOUND_CHOR_DIR) and generates the script lists included below. Run one
//! from disk while working on it: `chor game --overlay ~/projects/model-chor`.

mod image_chor {
    include!(concat!(env!("OUT_DIR"), "/image_chor.rs"));
}

mod model_chor {
    include!(concat!(env!("OUT_DIR"), "/model_chor.rs"));
}

mod edit_chor {
    include!(concat!(env!("OUT_DIR"), "/edit_chor.rs"));
}

mod sound_chor {
    include!(concat!(env!("OUT_DIR"), "/sound_chor.rs"));
}

/// The source editor's commit, or "" when it isn't built in.
pub use edit_chor::EDIT_REV;
/// The image editor's commit, or "" when it isn't built in.
pub use image_chor::PAINT_REV;
/// The model editor's commit, or "" when it isn't built in.
pub use model_chor::MODEL_REV;
/// The sound tool's commit, or "" when it isn't built in.
pub use sound_chor::SOUND_REV;

/// The scripts of a built-in app, keyed by the directory name it loads under.
/// None for a tool that isn't built in (no `studio`, or the build couldn't
/// fetch it).
pub fn scripts(directory: &str) -> Option<&'static [(&'static str, &'static str)]> {
    let list: &'static [(&'static str, &'static str)] = match directory {
        crate::command::PAINT_OVERLAY => image_chor::PAINT,
        crate::command::MODEL_OVERLAY => model_chor::MODEL,
        crate::command::EDIT_OVERLAY => edit_chor::EDIT,
        crate::command::SOUND_OVERLAY => sound_chor::SOUND,
        _ => return None,
    };
    (!list.is_empty()).then_some(list)
}

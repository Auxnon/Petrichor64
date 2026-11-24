//! Android-only glue that needs the Java side: the soft keyboard, how much of
//! the window it covers, app storage, and the share sheet.
//!
//! The app is a `NativeActivity` with no Java of its own, so everything here
//! calls framework classes through JNI from the engine's thread. None of it
//! can be exercised off a device: on a failure each function logs to logcat
//! and does nothing, rather than taking the app down.

use android_activity::AndroidApp;
use jni::objects::{JObject, JValue};
use jni::{JNIEnv, JavaVM};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

static APP: OnceLock<AndroidApp> = OnceLock::new();

/// Called once from `android_main`, before the event loop.
pub fn init(app: &AndroidApp) {
    let _ = APP.set(app.clone());
}

/// Where apps made with `new` live and `load` looks: the app's own external
/// files folder (reachable over USB as `Android/data/<package>/files`), or its
/// private one when there's no shared storage.
pub fn home() -> Option<PathBuf> {
    let app = APP.get()?;
    app.external_data_path()
        .or_else(|| app.internal_data_path())
}

type Jni<T> = Result<T, jni::errors::Error>;

/// Run `f` with a JNI environment and the activity object. A Java exception
/// is cleared (it would poison every later call) and reported as the error.
fn with_activity<T>(what: &str, f: impl FnOnce(&mut JNIEnv, &JObject) -> Jni<T>) -> Option<T> {
    let app = APP.get()?;
    // Safety: both pointers come from android-activity, which keeps the VM and
    // a global reference to the activity alive for the life of the process.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut _) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let activity = unsafe { JObject::from_raw(app.activity_as_ptr() as jni::sys::jobject) };
    match f(&mut env, &activity) {
        Ok(v) => Some(v),
        Err(e) => {
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_describe();
                let _ = env.exception_clear();
            }
            ::log::error!("petrichor64: {} failed: {}", what, e);
            None
        }
    }
}

fn decor_view<'a>(env: &mut JNIEnv<'a>, activity: &JObject) -> Jni<JObject<'a>> {
    let window = env
        .call_method(activity, "getWindow", "()Landroid/view/Window;", &[])?
        .l()?;
    env.call_method(&window, "getDecorView", "()Landroid/view/View;", &[])?
        .l()
}

/// Show or hide the soft keyboard. `NativeActivity`'s own call for this
/// (`ANativeActivity_showSoftInput`) does nothing on many devices, so the
/// input method manager is asked directly first.
pub fn keyboard(show: bool) {
    let done = with_activity("soft keyboard", |env, activity| {
        let name = env.new_string("input_method")?;
        let imm = env
            .call_method(
                activity,
                "getSystemService",
                "(Ljava/lang/String;)Ljava/lang/Object;",
                &[(&name).into()],
            )?
            .l()?;
        let view = decor_view(env, activity)?;
        if show {
            env.call_method(
                &imm,
                "showSoftInput",
                "(Landroid/view/View;I)Z",
                &[(&view).into(), JValue::Int(0)],
            )?
            .z()
        } else {
            let token = env
                .call_method(&view, "getWindowToken", "()Landroid/os/IBinder;", &[])?
                .l()?;
            env.call_method(
                &imm,
                "hideSoftInputFromWindow",
                "(Landroid/os/IBinder;I)Z",
                &[(&token).into(), JValue::Int(0)],
            )?
            .z()
        }
    });
    if done != Some(true) {
        if let Some(app) = APP.get() {
            if show {
                app.show_soft_input(true);
            } else {
                app.hide_soft_input(false);
            }
        }
    }
    // Ask again soon: the keyboard animates in.
    POLL.store(0, Ordering::Relaxed);
}

fn sdk(env: &mut JNIEnv) -> Jni<i32> {
    env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?
        .i()
}

fn keyboard_height_now() -> Option<f32> {
    with_activity("keyboard height", |env, activity| {
        let view = decor_view(env, activity)?;
        if sdk(env)? >= 30 {
            // The IME's own inset: 0 while it is hidden or floating.
            let insets = env
                .call_method(
                    &view,
                    "getRootWindowInsets",
                    "()Landroid/view/WindowInsets;",
                    &[],
                )?
                .l()?;
            if insets.is_null() {
                return Ok(0.);
            }
            let ime = env
                .call_static_method("android/view/WindowInsets$Type", "ime", "()I", &[])?
                .i()?;
            let inset = env
                .call_method(
                    &insets,
                    "getInsets",
                    "(I)Landroid/graphics/Insets;",
                    &[JValue::Int(ime)],
                )?
                .l()?;
            Ok(env.get_field(&inset, "bottom", "I")?.i()? as f32)
        } else {
            // Older: what's left of the window once the keyboard has taken
            // its part.
            let rect = env.new_object("android/graphics/Rect", "()V", &[])?;
            env.call_method(
                &view,
                "getWindowVisibleDisplayFrame",
                "(Landroid/graphics/Rect;)V",
                &[(&rect).into()],
            )?;
            let bottom = env.get_field(&rect, "bottom", "I")?.i()?;
            let root = env
                .call_method(&view, "getRootView", "()Landroid/view/View;", &[])?
                .l()?;
            let height = env.call_method(&root, "getHeight", "()I", &[])?.i()?;
            Ok((height - bottom).max(0) as f32)
        }
    })
}

static POLL: AtomicU32 = AtomicU32::new(0);
static HEIGHT: AtomicU32 = AtomicU32::new(0);

/// Window pixels the docked keyboard covers at the bottom, 0 for none or a
/// floating one. Called every frame while the console is open; asks Java
/// every sixth call and answers from the last reading otherwise.
pub fn keyboard_height() -> f32 {
    if POLL.fetch_add(1, Ordering::Relaxed) % 6 == 0 {
        if let Some(h) = keyboard_height_now() {
            HEIGHT.store(h.to_bits(), Ordering::Relaxed);
        }
    }
    f32::from_bits(HEIGHT.load(Ordering::Relaxed))
}

/// Offer a file the engine wrote to other apps. It is copied into the shared
/// Downloads collection (`Download/Petrichor64/`), which gives it a
/// `content://` address other apps may read without this one declaring a file
/// provider, and then the system's share sheet is opened on that. Needs
/// Android 10; before that the file stays where it was written and this says
/// where.
pub fn share(path: &Path, mime: &str) -> Result<(), String> {
    let data = std::fs::read(path).map_err(|e| e.to_string())?;
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or("no file name")?
        .to_string();
    let shared = with_activity("share", |env, activity| {
        if sdk(env)? < 29 {
            return Ok(false);
        }
        let resolver = env
            .call_method(
                activity,
                "getContentResolver",
                "()Landroid/content/ContentResolver;",
                &[],
            )?
            .l()?;
        let values = env.new_object("android/content/ContentValues", "()V", &[])?;
        for (k, v) in [
            ("_display_name", name.as_str()),
            ("mime_type", mime),
            ("relative_path", "Download/Petrichor64"),
        ] {
            let k = env.new_string(k)?;
            let v = env.new_string(v)?;
            env.call_method(
                &values,
                "put",
                "(Ljava/lang/String;Ljava/lang/String;)V",
                &[(&k).into(), (&v).into()],
            )?;
        }
        let downloads = env
            .get_static_field(
                "android/provider/MediaStore$Downloads",
                "EXTERNAL_CONTENT_URI",
                "Landroid/net/Uri;",
            )?
            .l()?;
        let uri = env
            .call_method(
                &resolver,
                "insert",
                "(Landroid/net/Uri;Landroid/content/ContentValues;)Landroid/net/Uri;",
                &[(&downloads).into(), (&values).into()],
            )?
            .l()?;
        if uri.is_null() {
            return Err(jni::errors::Error::NullPtr("MediaStore insert"));
        }
        let stream = env
            .call_method(
                &resolver,
                "openOutputStream",
                "(Landroid/net/Uri;)Ljava/io/OutputStream;",
                &[(&uri).into()],
            )?
            .l()?;
        let bytes = env.byte_array_from_slice(&data)?;
        env.call_method(&stream, "write", "([B)V", &[(&bytes).into()])?;
        env.call_method(&stream, "close", "()V", &[])?;

        let action = env.new_string("android.intent.action.SEND")?;
        let intent = env.new_object(
            "android/content/Intent",
            "(Ljava/lang/String;)V",
            &[(&action).into()],
        )?;
        let kind = env.new_string(mime)?;
        env.call_method(
            &intent,
            "setType",
            "(Ljava/lang/String;)Landroid/content/Intent;",
            &[(&kind).into()],
        )?;
        let extra = env.new_string("android.intent.extra.STREAM")?;
        env.call_method(
            &intent,
            "putExtra",
            "(Ljava/lang/String;Landroid/os/Parcelable;)Landroid/content/Intent;",
            &[(&extra).into(), (&uri).into()],
        )?;
        // FLAG_GRANT_READ_URI_PERMISSION
        env.call_method(
            &intent,
            "addFlags",
            "(I)Landroid/content/Intent;",
            &[JValue::Int(1)],
        )?;
        let title = env.new_string(&name)?;
        let chooser = env
            .call_static_method(
                "android/content/Intent",
                "createChooser",
                "(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;",
                &[(&intent).into(), (&title).into()],
            )?
            .l()?;
        env.call_method(
            activity,
            "startActivity",
            "(Landroid/content/Intent;)V",
            &[(&chooser).into()],
        )?;
        Ok(true)
    });
    match shared {
        Some(true) => Ok(()),
        Some(false) => Err(format!(
            "sharing needs Android 10; the file is at {}",
            path.display()
        )),
        None => Err(format!(
            "the share sheet couldn't open; the file is at {}",
            path.display()
        )),
    }
}

// --- The Java bridge: drags from other apps, and the document picker -------

/// `android/Bridge.java`, compiled by build.rs. Empty when the build had no
/// Android SDK or JDK to compile it with (`just check-android`), and then
/// drags and the picker do nothing.
const DEX: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/bridge.dex"));

/// Paths Java has handed over, waiting for the engine's thread.
static FILES: std::sync::Mutex<Vec<String>> = std::sync::Mutex::new(Vec::new());
static BRIDGE: OnceLock<Option<jni::objects::GlobalRef>> = OnceLock::new();

/// `Bridge.onFile(path)`, called on a Java thread.
extern "system" fn on_file(
    mut env: JNIEnv,
    _class: jni::objects::JClass,
    path: jni::objects::JString,
) {
    if let Ok(s) = env.get_string(&path) {
        if let Ok(mut files) = FILES.lock() {
            files.push(s.into());
        }
    }
}

/// Load the bridge class out of memory (the APK holds no dex, and a class
/// loader over a byte buffer needs none) and hook up `onFile`.
fn bridge() -> Option<&'static jni::objects::GlobalRef> {
    BRIDGE
        .get_or_init(|| {
            if DEX.is_empty() {
                ::log::warn!("petrichor64: built without the Java bridge: no drag and drop, no file picker");
                return None;
            }
            with_activity("java bridge", |env, activity| {
                // The loader keeps reading the buffer, so it is never freed.
                let bytes: &'static mut [u8] = Box::leak(DEX.to_vec().into_boxed_slice());
                // Safety: the slice lives for the rest of the process.
                let buffer =
                    unsafe { env.new_direct_byte_buffer(bytes.as_mut_ptr(), bytes.len())? };
                let parent = env
                    .call_method(activity, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?
                    .l()?;
                let loader = env.new_object(
                    "dalvik/system/InMemoryDexClassLoader",
                    "(Ljava/nio/ByteBuffer;Ljava/lang/ClassLoader;)V",
                    &[(&buffer).into(), (&parent).into()],
                )?;
                let name = env.new_string("com.makeavoy.petrichor64.Bridge")?;
                let class = env
                    .call_method(
                        &loader,
                        "loadClass",
                        "(Ljava/lang/String;)Ljava/lang/Class;",
                        &[(&name).into()],
                    )?
                    .l()?;
                let class = jni::objects::JClass::from(class);
                env.register_native_methods(
                    &class,
                    &[jni::NativeMethod {
                        name: "onFile".into(),
                        sig: "(Ljava/lang/String;)V".into(),
                        fn_ptr: on_file as *mut std::ffi::c_void,
                    }],
                )?;
                env.new_global_ref(class)
            })
        })
        .as_ref()
}

/// Call one of the bridge's `static void name(Activity)` methods.
fn bridge_call(name: &'static str) {
    let Some(class) = bridge() else { return };
    with_activity(name, |env, activity| {
        let class: &jni::objects::JClass = class.as_obj().into();
        env.call_static_method(
            class,
            name,
            "(Landroid/app/Activity;)V",
            &[activity.into()],
        )?
        .v()
    });
}

/// Start taking drags from other apps as drops. Called once the window is up.
pub fn accept_drops() {
    bridge_call("install");
}

/// Show the system's document picker (`io.open`). What's chosen turns up in
/// `files`.
pub fn pick() {
    bridge_call("pick");
}

/// Files dragged in or picked since the last call, as paths in the app's
/// cache. Called every frame, so it takes the lock only to look.
pub fn files() -> Vec<String> {
    match FILES.try_lock() {
        Ok(mut files) if !files.is_empty() => std::mem::take(&mut *files),
        _ => Vec::new(),
    }
}

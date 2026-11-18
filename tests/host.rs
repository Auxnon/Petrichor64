//! Two engine instances in one process, on one wgpu device, with no window:
//! the host API end to end. Needs a GPU adapter (any backend, a software one
//! will do); without one the test says so and passes.
#![cfg(all(feature = "headed", not(target_arch = "wasm32")))]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use petrichor64::host::{Cart, HostGpu, Input, Options, Policy, Request};
use petrichor64::winit::keyboard::KeyCode;
use petrichor64::{wgpu, Instance};

/// Reports what it can reach, echoes host messages, says when `a` goes down,
/// and asks for a window title.
const CART: &str = r#"
function main()
    host.send("reach conn=" .. type(conn) .. " io=" .. type(io) .. " sub=" .. type(sub))
    attr { title = "probe" }
    sky:fill("f00")
end
DOWN = false
function loop()
    local m = host.recv()
    while m do
        host.send("echo " .. m)
        m = host.recv()
    end
    local now = key("a")
    if now ~= DOWN then
        DOWN = now
        host.send(now and "a down" or "a up")
    end
end
"#;

/// Hook events: says when each arrives and how many loops had run, says what
/// the loop after it saw, answers `echo` with a `host.hook`, and turns the sky
/// green on `paint` (seen even where `host` is nil, in a sandbox).
/// silt-safe style (AGENTS.md): calls hoisted out of concatenations.
const HOOK_CART: &str = r#"
function main()
    S = { loops = 0, last = 'none', fresh = false }
    sky:fill("f00")
    if host then
        host.send("ready")
    end
end
function hook(key, value)
    S.last = key .. "=" .. value
    S.fresh = true
    if key == "paint" then
        sky:fill("0f0")
    end
    if host then
        local n = tostring(S.loops)
        local m = "hook " .. S.last .. " at " .. n
        host.send(m)
        if key == "echo" then
            host.hook("echoed", value)
            host.hook("bare")
        end
    end
end
function loop()
    S.loops = S.loops + 1
    if S.fresh then
        S.fresh = false
        if host then
            local n = tostring(S.loops)
            local m = "loop " .. n .. " saw " .. S.last
            host.send(m)
        end
    end
end
"#;

fn cart_dir(name: &str) -> PathBuf {
    cart_dir_with(name, CART)
}

fn cart_dir_with(name: &str, src: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p64-host-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    std::fs::write(dir.join("scripts/main.lua"), src).unwrap();
    // `pack` wants an icon in the cart's root: the png the zip rides on.
    std::fs::copy(
        concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/assets/petrichor-small-icon.png"
        ),
        dir.join("icon.png"),
    )
    .unwrap();
    dir
}

fn gpu() -> Option<HostGpu> {
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    Some(HostGpu {
        device,
        queue,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
    })
}

/// Tick and draw every instance at the engine's pace until `done`, or fail.
fn run(
    insts: &mut [Instance],
    heard: &mut [Vec<String>],
    start: Instant,
    what: &str,
    done: impl Fn(&[Vec<String>]) -> bool,
) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        for (inst, heard) in insts.iter_mut().zip(heard.iter_mut()) {
            if inst.frame(start.elapsed()) {
                inst.render();
            }
            heard.extend(inst.messages());
        }
        if done(heard) {
            return;
        }
        assert!(
            Instant::now() < until,
            "timed out waiting for {what}: heard {heard:?}"
        );
        std::thread::sleep(Duration::from_millis(4));
    }
}

fn has(heard: &[String], msg: &str) -> bool {
    heard.iter().any(|m| m == msg)
}

/// The middle pixel of an instance's texture.
fn middle_pixel(gpu: &HostGpu, inst: &Instance, size: (u32, u32)) -> [u8; 4] {
    let texture = inst.texture().texture();
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d {
                x: size.0 / 2,
                y: size.1 / 2,
                z: 0,
            },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(256),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..4);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .expect("poll");
    let data = slice.get_mapped_range().expect("mapped");
    [data[0], data[1], data[2], data[3]]
}

#[test]
fn two_instances_side_by_side() {
    let Some(gpu) = gpu() else {
        eprintln!("no GPU adapter: skipping the host test");
        return;
    };
    let size = (320, 240);
    let quiet = |policy: Policy| Policy {
        audio: false,
        ..policy
    };
    // An open cart from a folder, and a sandboxed one (but for the host
    // channel, so it can report) from bytes.
    let dir = cart_dir("a");
    let packed = dir.with_extension("game.png");
    petrichor64::pack(&dir.to_string_lossy(), Some(&packed.to_string_lossy()))
        .expect("packing the test cart");
    let bytes = std::fs::read(&packed).unwrap();
    let options = |cart, policy| Options {
        cart,
        size,
        scale: 1.0,
        policy,
    };
    let mut insts = vec![
        Instance::new(
            &gpu,
            options(Cart::Path(dir.clone()), quiet(Policy::open())),
        )
        .unwrap(),
        Instance::new(
            &gpu,
            options(
                Cart::Bytes(bytes),
                quiet(Policy {
                    host_messages: true,
                    ..Policy::sandbox()
                }),
            ),
        )
        .unwrap(),
    ];
    let mut heard = vec![Vec::new(), Vec::new()];
    let start = Instant::now();

    // Each cart reaches what its policy allows.
    run(&mut insts, &mut heard, start, "both carts to start", |h| {
        h.iter().all(|h| h.iter().any(|m| m.starts_with("reach")))
    });
    assert!(
        has(&heard[0], "reach conn=function io=table sub=function"),
        "{heard:?}"
    );
    assert!(has(&heard[1], "reach conn=nil io=nil sub=nil"), "{heard:?}");

    // What a cart wants of a window arrives as a request, to grant or not.
    let mut titled = false;
    for _ in 0..200 {
        insts[0].frame(start.elapsed());
        titled |= insts[0]
            .requests()
            .contains(&Request::Title("probe".into()));
        if titled {
            break;
        }
        std::thread::sleep(Duration::from_millis(4));
    }
    assert!(titled, "the title request never came");

    // Host messages go to the instance they were posted to.
    insts[1].post("ping".into());
    run(&mut insts, &mut heard, start, "the echo", |h| {
        has(&h[1], "echo ping")
    });
    assert!(!has(&heard[0], "echo ping"));

    // Input goes to the instance it was given to, and no other.
    insts[0].input(Input::Key {
        code: KeyCode::KeyA,
        pressed: true,
    });
    run(&mut insts, &mut heard, start, "the key", |h| {
        has(&h[0], "a down")
    });
    insts[0].input(Input::FocusLost);
    run(&mut insts, &mut heard, start, "the key let go", |h| {
        has(&h[0], "a up")
    });
    assert!(!heard[1].iter().any(|m| m.starts_with("a ")), "{heard:?}");

    // Both draw into textures of their own, of the size asked for.
    for inst in &insts {
        let t = inst.texture().texture();
        assert_eq!((t.width(), t.height()), size);
        let px = middle_pixel(&gpu, inst, size);
        assert!(
            px[0] > 200 && px[1] < 60 && px[2] < 60,
            "the red sky, got {px:?}"
        );
    }
    insts[1].resize((200, 100), 1.0);
    let t = insts[1].texture().texture();
    assert_eq!((t.width(), t.height()), (200, 100));

    drop(insts);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&packed);
}

/// Tick and draw one instance until `done` holds for what it said (messages)
/// and hooked (`hooks()`), or fail.
fn run_hooks(
    inst: &mut Instance,
    heard: &mut Vec<String>,
    hooked: &mut Vec<(String, String)>,
    start: Instant,
    what: &str,
    done: impl Fn(&[String], &[(String, String)]) -> bool,
) {
    let until = Instant::now() + Duration::from_secs(20);
    loop {
        if inst.frame(start.elapsed()) {
            inst.render();
        }
        heard.extend(inst.messages());
        hooked.extend(inst.hooks());
        if done(heard, hooked) {
            return;
        }
        assert!(
            Instant::now() < until,
            "timed out waiting for {what}: heard {heard:?}, hooked {hooked:?}"
        );
        std::thread::sleep(Duration::from_millis(4));
    }
}

/// The loop count in "hook <k>=<v> at <n>", for the event `kv`.
fn hook_at(heard: &[String], kv: &str) -> Option<u64> {
    let head = format!("hook {kv} at ");
    heard
        .iter()
        .find_map(|m| m.strip_prefix(&head))
        .map(|n| n.parse().expect("a loop count"))
}

#[test]
fn hooks_both_ways() {
    let Some(gpu) = gpu() else {
        eprintln!("no GPU adapter: skipping the hook test");
        return;
    };
    let size = (64, 48);
    let quiet = |policy: Policy| Policy {
        audio: false,
        ..policy
    };
    let options = |cart, policy| Options {
        cart,
        size,
        scale: 1.0,
        policy,
    };
    let hook_dir = cart_dir_with("hooks", HOOK_CART);
    let plain_dir = cart_dir("hookless");
    let start = Instant::now();

    let mut inst = Instance::new(
        &gpu,
        options(Cart::Path(hook_dir.clone()), quiet(Policy::open())),
    )
    .unwrap();
    // Posted before the cart has run at all: kept for it, and after main().
    assert!(inst.hook("early", "1"));
    let (mut heard, mut hooked) = (Vec::new(), Vec::new());
    run_hooks(
        &mut inst,
        &mut heard,
        &mut hooked,
        start,
        "the early hook",
        |h, _| {
            h.iter()
                .any(|m| m.starts_with("loop ") && m.ends_with(" saw early=1"))
        },
    );
    assert_eq!(
        heard.first().map(String::as_str),
        Some("ready"),
        "{heard:?}"
    );

    // One event: the loop right after it sees it.
    assert!(inst.hook("search", "oak plank"));
    run_hooks(
        &mut inst,
        &mut heard,
        &mut hooked,
        start,
        "the search hook",
        |h, _| h.iter().any(|m| m.ends_with(" saw search=oak plank")),
    );
    let at = hook_at(&heard, "search=oak plank").expect("the hook ran");
    let next = format!("loop {} saw search=oak plank", at + 1);
    assert!(heard.contains(&next), "wanted {next:?} in {heard:?}");

    // Several between two frames: all of them, in order, before one loop.
    heard.clear();
    for k in ["a", "b", "c"] {
        assert!(inst.hook(k, k.to_uppercase()));
    }
    run_hooks(
        &mut inst,
        &mut heard,
        &mut hooked,
        start,
        "three hooks",
        |h, _| h.iter().any(|m| m.ends_with(" saw c=C")),
    );
    let hooks: Vec<&String> = heard.iter().filter(|m| m.starts_with("hook ")).collect();
    assert_eq!(hooks.len(), 3, "{heard:?}");
    let at = hook_at(&heard, "a=A").unwrap();
    for (m, kv) in hooks.iter().zip(["a=A", "b=B", "c=C"]) {
        assert_eq!(**m, format!("hook {kv} at {at}"), "{heard:?}");
    }
    let next = format!("loop {} saw c=C", at + 1);
    assert!(heard.contains(&next), "wanted {next:?} in {heard:?}");

    // Out: `host.hook` reaches `hooks()`, value defaulting to "".
    assert!(inst.hook("echo", "plank"));
    run_hooks(
        &mut inst,
        &mut heard,
        &mut hooked,
        start,
        "host.hook",
        |_, k| k.len() >= 2,
    );
    assert_eq!(
        hooked,
        vec![
            ("echoed".to_string(), "plank".to_string()),
            ("bare".to_string(), String::new())
        ]
    );

    // Past the caps, dropped.
    assert!(!inst.hook("k".repeat(petrichor64::host::HOOK_KEY_MAX + 1), ""));
    assert!(!inst.hook("k", "v".repeat(petrichor64::host::HOOK_VALUE_MAX + 1)));

    // `paint` turns the sky green: the check the sandbox below relies on.
    assert!(inst.hook("paint", ""));
    run_hooks(
        &mut inst,
        &mut heard,
        &mut hooked,
        start,
        "paint",
        |h, _| h.iter().any(|m| m.ends_with(" saw paint=")),
    );
    for _ in 0..10 {
        if inst.frame(start.elapsed()) {
            inst.render();
        }
        std::thread::sleep(Duration::from_millis(17));
    }
    let px = middle_pixel(&gpu, &inst, size);
    assert!(px[1] > 200 && px[0] < 60, "the green sky, got {px:?}");
    drop(inst);

    // Sandboxed: no host channel, so no hooks either way.
    let packed = hook_dir.with_extension("game.png");
    petrichor64::pack(&hook_dir.to_string_lossy(), Some(&packed.to_string_lossy()))
        .expect("packing the hook cart");
    let bytes = std::fs::read(&packed).unwrap();
    let mut boxed =
        Instance::new(&gpu, options(Cart::Bytes(bytes), quiet(Policy::sandbox()))).unwrap();
    assert!(!boxed.hook("paint", ""), "a sandbox took a hook");
    let until = Instant::now() + Duration::from_millis(1500);
    let mut out = Vec::new();
    while Instant::now() < until {
        if boxed.frame(start.elapsed()) {
            boxed.render();
        }
        assert!(!boxed.hook("paint", ""));
        out.extend(boxed.hooks());
        assert!(boxed.messages().is_empty());
        std::thread::sleep(Duration::from_millis(4));
    }
    assert!(out.is_empty(), "{out:?}");
    let px = middle_pixel(&gpu, &boxed, size);
    assert!(px[0] > 200 && px[1] < 60, "still the red sky, got {px:?}");
    drop(boxed);

    // A cart with no `hook` doesn't hear them, and carries on.
    let mut plain = Instance::new(
        &gpu,
        options(Cart::Path(plain_dir.clone()), quiet(Policy::open())),
    )
    .unwrap();
    let mut heard = Vec::new();
    let mut hooked = Vec::new();
    for k in ["search", "show"] {
        assert!(plain.hook(k, "x"));
    }
    plain.post("ping".into());
    run_hooks(
        &mut plain,
        &mut heard,
        &mut hooked,
        start,
        "the echo",
        |h, _| h.iter().any(|m| m == "echo ping"),
    );
    assert!(hooked.is_empty(), "{hooked:?}");
    plain.post("pong".into());
    run_hooks(
        &mut plain,
        &mut heard,
        &mut hooked,
        start,
        "a second echo",
        |h, _| h.iter().any(|m| m == "echo pong"),
    );
    drop(plain);

    let _ = std::fs::remove_dir_all(&hook_dir);
    let _ = std::fs::remove_dir_all(&plain_dir);
    let _ = std::fs::remove_file(&packed);
}

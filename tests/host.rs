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

fn cart_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("p64-host-test-{}-{name}", std::process::id()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::create_dir_all(dir.join("scripts")).unwrap();
    std::fs::write(dir.join("scripts/main.lua"), CART).unwrap();
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
    pixel_at(gpu, inst, (size.0 / 2, size.1 / 2))
}

fn pixel_at(gpu: &HostGpu, inst: &Instance, at: (u32, u32)) -> [u8; 4] {
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
                x: at.0,
                y: at.1,
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

/// A click at a pixel of the instance.
#[cfg(feature = "studio")]
fn click(inst: &mut Instance, x: f32, y: f32) {
    use petrichor64::host::Button;
    inst.input(Input::PointerMoved { x, y });
    for pressed in [true, false] {
        inst.input(Input::Button {
            button: Button::Left,
            pressed,
        });
    }
}

/// Tick and draw a few frames, collecting what the instance asked for.
#[cfg(feature = "studio")]
fn settle(inst: &mut Instance, start: Instant) -> Vec<Request> {
    let mut asked = vec![];
    let until = Instant::now() + Duration::from_millis(400);
    while Instant::now() < until {
        if inst.frame(start.elapsed()) {
            inst.render();
        }
        asked.extend(inst.requests());
        std::thread::sleep(Duration::from_millis(4));
    }
    asked
}

/// The gear in the corner opens the console and the command strip and asks
/// for the keyboard; the strip's share button packs the app. A cart with no
/// console gets no gear.
#[cfg(feature = "studio")]
#[test]
fn the_gear_opens_the_console_and_strip() {
    let Some(gpu) = gpu() else {
        eprintln!("no GPU adapter: skipping the strip test");
        return;
    };
    let size = (320u32, 240u32);
    let (w, h) = (size.0 as f32, size.1 as f32);
    let dir = cart_dir("strip");
    let options = |policy| Options {
        cart: Cart::Path(dir.clone()),
        size,
        scale: 1.0,
        policy,
    };
    let quiet = |policy: Policy| Policy {
        audio: false,
        ..policy
    };
    let mut inst = Instance::new(&gpu, options(quiet(Policy::open()))).unwrap();
    let start = Instant::now();
    let mut heard = vec![Vec::new()];
    run(
        std::slice::from_mut(&mut inst),
        &mut heard,
        start,
        "the cart to start",
        |h| h[0].iter().any(|m| m.starts_with("reach")),
    );
    settle(&mut inst, start);
    // The far left of the bottom row is the app's red sky until the strip
    // is out, and the strip's dark bar after.
    let bar = (3, size.1 - 3);
    let red = |px: [u8; 4]| px[0] > 150 && px[1] < 80;
    assert!(red(pixel_at(&gpu, &inst, bar)), "sky before the gear");

    // The gear: bottom right corner.
    click(&mut inst, w - 4., h - 4.);
    let asked = settle(&mut inst, start);
    assert!(
        asked.contains(&Request::Keyboard(true)),
        "the gear asks for the keyboard: {asked:?}"
    );
    let px = pixel_at(&gpu, &inst, bar);
    assert!(!red(px), "the strip's bar is drawn, got {px:?}");

    // Share is the last of five centred tools; its choices are three, with
    // the zip in the middle. Cells are 12 gui pixels: find how wide that is
    // here by trying, since the gui layer needn't be the window's size.
    let mut shared = None;
    for cell in [12., 16., 20., 24., 28.] {
        click(&mut inst, w / 2. + cell * 2., h - 4.);
        settle(&mut inst, start);
        click(&mut inst, w / 2., h - 4.);
        let asked = settle(&mut inst, start);
        shared = asked.into_iter().find_map(|r| match r {
            Request::Share { path, mime } => Some((path, mime)),
            _ => None,
        });
        if shared.is_some() {
            break;
        }
    }
    let (path, mime) = shared.expect("share then zip asks the host to share a file");
    assert_eq!(mime, "application/zip");
    assert_eq!(path, dir.with_extension("zip"));
    let zip = std::fs::read(&path).expect("the zip was written");
    assert_eq!(&zip[..2], b"PK");
    let name = format!(
        "{}/scripts/main.lua",
        dir.file_name().unwrap().to_string_lossy()
    );
    assert!(
        zip.windows(name.len()).any(|w| w == name.as_bytes()),
        "the zip holds {name}"
    );

    // The picture next to it is a game: the icon with the same files zipped
    // behind it, which another instance loads from bytes.
    let cell = (0..5)
        .map(|i| 12. + 4. * i as f32)
        .find(|cell| {
            click(&mut inst, w / 2. + cell * 2., h - 4.);
            settle(&mut inst, start);
            click(&mut inst, w / 2. - cell, h - 4.);
            let asked = settle(&mut inst, start);
            asked
                .iter()
                .any(|r| matches!(r, Request::Share { mime, .. } if *mime == "image/png"))
        })
        .expect("share then picture asks the host to share a png");
    let _ = cell;
    let game = dir.with_extension("game.png");
    let bytes = std::fs::read(&game).expect("the game.png was written");
    assert_eq!(&bytes[1..4], b"PNG");
    let mut loaded = Instance::new(
        &gpu,
        Options {
            cart: Cart::Bytes(bytes),
            size,
            scale: 1.0,
            policy: quiet(Policy::open()),
        },
    )
    .unwrap();
    let mut heard2 = vec![Vec::new()];
    run(
        std::slice::from_mut(&mut loaded),
        &mut heard2,
        start,
        "the exported game to start",
        |h| h[0].iter().any(|m| m.starts_with("reach")),
    );
    drop(loaded);
    let _ = std::fs::remove_file(&game);

    // The gear again puts it all away.
    click(&mut inst, w - 4., h - 4.);
    let asked = settle(&mut inst, start);
    assert!(asked.contains(&Request::Keyboard(false)), "{asked:?}");
    assert!(red(pixel_at(&gpu, &inst, bar)), "sky after closing");
    drop(inst);

    // No console, no gear: the press goes to the cart and nothing is asked.
    let mut inst = Instance::new(
        &gpu,
        options(quiet(Policy {
            host_messages: true,
            ..Policy::sandbox()
        })),
    )
    .unwrap();
    let mut heard = vec![Vec::new()];
    run(
        std::slice::from_mut(&mut inst),
        &mut heard,
        start,
        "the sandboxed cart to start",
        |h| h[0].iter().any(|m| m.starts_with("reach")),
    );
    click(&mut inst, w - 4., h - 4.);
    let asked = settle(&mut inst, start);
    assert!(
        !asked.iter().any(|r| matches!(r, Request::Keyboard(_))),
        "{asked:?}"
    );
    drop(inst);
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_file(&path);
}

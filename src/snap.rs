//! Frame grabs for testing without a screen.
//!
//! `CHOR_SNAP=/some/prefix CHOR_SNAP_AT=120,300` saves the rendered scene at
//! those frame counts (`Global::iteration`) as `/some/prefix-120.png`, and so
//! on. It reads the post texture — the 3D scene, front pass and (unless the
//! modern monitor puts the gui on top) the gui layers, before the CRT shader —
//! so it works on a locked or headless session where a compositor screenshot
//! only sees the lock screen. Costs one env lookup at startup when unset.

use std::sync::OnceLock;

static PLAN: OnceLock<Option<(String, Vec<u64>)>> = OnceLock::new();

fn plan() -> &'static Option<(String, Vec<u64>)> {
    PLAN.get_or_init(|| {
        let prefix = std::env::var("CHOR_SNAP").ok()?;
        let at = std::env::var("CHOR_SNAP_AT").unwrap_or_else(|_| "120".into());
        let frames = at.split(',').filter_map(|s| s.trim().parse().ok()).collect();
        Some((prefix, frames))
    })
}

/// Is CHOR_SNAP set at all?
pub fn enabled() -> bool {
    plan().is_some()
}

/// The file to save this frame to, if one was asked for.
pub fn wanted(iteration: u64) -> Option<String> {
    let (prefix, frames) = plan().as_ref()?;
    frames
        .contains(&iteration)
        .then(|| format!("{}-{}.png", prefix, iteration))
}

pub struct Pending {
    buffer: wgpu::Buffer,
    width: u32,
    height: u32,
    padded_row: u32,
    bgra: bool,
    path: String,
}

/// Record a copy of `texture` into a readable buffer on `encoder`.
pub fn record(
    device: &wgpu::Device,
    encoder: &mut wgpu::CommandEncoder,
    texture: &wgpu::Texture,
    path: String,
) -> Pending {
    let format = texture.format();
    let width = texture.width();
    let height = texture.height();
    let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let padded_row = (width * 4).div_ceil(align) * align;
    let buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("snap"),
        size: (padded_row * height) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded_row),
                rows_per_image: Some(height),
            },
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    let bgra = matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
    );
    Pending {
        buffer,
        width,
        height,
        padded_row,
        bgra,
        path,
    }
}

/// After the submit: wait for the copy, then write the PNG.
pub fn save(device: &wgpu::Device, p: Pending) {
    let slice = p.buffer.slice(..);
    slice.map_async(wgpu::MapMode::Read, |_| {});
    if device.poll(wgpu::PollType::wait_indefinitely()).is_err() {
        eprintln!("snap: poll failed for {}", p.path);
        return;
    }
    let data = slice.get_mapped_range();
    let mut img = image::RgbaImage::new(p.width, p.height);
    for y in 0..p.height {
        let row = &data[(y * p.padded_row) as usize..][..(p.width * 4) as usize];
        for x in 0..p.width {
            let i = (x * 4) as usize;
            let (r, b) = if p.bgra {
                (row[i + 2], row[i])
            } else {
                (row[i], row[i + 2])
            };
            img.put_pixel(x, y, image::Rgba([r, row[i + 1], b, 255]));
        }
    }
    drop(data);
    p.buffer.unmap();
    match img.save(&p.path) {
        Ok(()) => println!("snap: saved {}", p.path),
        Err(e) => eprintln!("snap: {}: {}", p.path, e),
    }
}

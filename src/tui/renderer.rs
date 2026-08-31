use crate::model::Vertex;
use crate::root::Core;
use glam::{vec3, vec4, Mat4, Vec3, Vec4};
use std::io::{stdout, Write};

/// Software rasterizer + terminal presenter for the `render-tui` backend.
///
/// Unlike the wgpu backend, this reads world/entity data directly (chunk
/// `vert_data`/`ind_data`, `LuaEnt` positions) rather than through cooked GPU
/// buffers — see the plan notes on why chunk/entity types weren't shared
/// between backends. Terrain gets real per-triangle geometry; entities are
/// drawn as coarse cube markers at their world position (no per-model shape
/// or rotation yet — a reasonable v1 cut given the "world view only" scope).
pub struct TuiRenderer {
    cols: u16,
    rows: u16,
}

struct Framebuffer {
    width: usize,
    height: usize,
    color: Vec<[u8; 3]>,
    depth: Vec<f32>,
}

const SKY_COLOR: [u8; 3] = [12, 12, 24];

impl Framebuffer {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            color: vec![SKY_COLOR; width * height],
            depth: vec![f32::INFINITY; width * height],
        }
    }

    fn set(&mut self, x: i32, y: i32, depth: f32, color: [u8; 3]) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        let i = y as usize * self.width + x as usize;
        if depth < self.depth[i] {
            self.depth[i] = depth;
            self.color[i] = color;
        }
    }
}

impl TuiRenderer {
    pub fn new() -> Self {
        let (cols, rows) = crossterm::terminal::size().unwrap_or((80, 24));
        Self { cols, rows }
    }

    pub fn render_frame(&mut self, core: &mut Core) {
        if let Ok((cols, rows)) = crossterm::terminal::size() {
            self.cols = cols;
            // Leave the bottom row for a status line so the frame never
            // scrolls the terminal.
            self.rows = rows.saturating_sub(1).max(1);
        }
        let w = self.cols as usize;
        let h = (self.rows as usize) * 2; // half-block trick: 2 pixel rows / char row
        if w == 0 || h == 0 {
            return;
        }
        let mut fb = Framebuffer::new(w, h);

        let cam_pos = core.global.cam_pos * 16.0;
        let mouse = core.global.smooth_cam_rot;
        let vp = camera_matrix(w as f32 / h as f32, cam_pos, mouse);

        for chunk in core.world.get_chunk_models() {
            if chunk.ind_data.len() < 3 || chunk.vert_data.is_empty() {
                continue;
            }
            let mvp = vp * Mat4::from_translation(chunk.pos.as_vec3() * 16.0);
            for tri in chunk.ind_data.chunks_exact(3) {
                let verts = [
                    &chunk.vert_data[tri[0] as usize],
                    &chunk.vert_data[tri[1] as usize],
                    &chunk.vert_data[tri[2] as usize],
                ];
                rasterize_tri(&mut fb, &mvp, verts, [90, 165, 90]);
            }
        }

        if let Some((cube_verts, cube_inds)) = &core.model_manager.CUBE.data {
            for wrapper in core
                .ent_manager
                .bundles
                .values()
                .flat_map(|b| b.array.iter())
            {
                let _ = wrapper.downcast_ref::<crate::lua_ent::LuaEnt, _, _>(|lent| {
                    if lent.get_flags() & crate::lua_ent::lua_ent_flags::DEAD
                        == crate::lua_ent::lua_ent_flags::DEAD
                    {
                        return Ok(());
                    }
                    let pos = vec3(lent.x as f32, lent.y as f32, lent.z as f32) * 16.0;
                    let scale = (lent.scale as f32).max(0.1);
                    let mvp = vp
                        * Mat4::from_translation(pos)
                        * Mat4::from_scale(Vec3::splat(scale));
                    for tri in cube_inds.chunks_exact(3) {
                        let verts = [
                            &cube_verts[tri[0] as usize],
                            &cube_verts[tri[1] as usize],
                            &cube_verts[tri[2] as usize],
                        ];
                        rasterize_tri(&mut fb, &mvp, verts, [220, 140, 60]);
                    }
                    Ok(())
                });
            }
        }

        present(&fb, self.cols, self.rows, core.global.fps as f32);
    }
}

/// Mirrors `render::generate_matrix`'s camera math (that module is wgpu-only
/// and not compiled under `render-tui`) combined straight to a view-projection
/// matrix, since this rasterizer only needs the combined transform.
fn camera_matrix(aspect: f32, mut camera_pos: Vec3, mouse: glam::Vec2) -> Mat4 {
    let persp = Mat4::perspective_rh(0.785398, aspect.max(0.01), 1., 24800.0);
    camera_pos *= 1.0; // already scaled by caller (matches render.rs's *16 on cam_pos)
    let r = mouse.x;
    let azimuth = mouse.y;
    let az = azimuth.cos() * 100.;
    let c = vec3(r.cos() * az, r.sin() * az, azimuth.sin() * 100.);
    let view = Mat4::look_at_rh(camera_pos, c + camera_pos, Vec3::Z);
    persp * view
}

fn rasterize_tri(fb: &mut Framebuffer, mvp: &Mat4, verts: [&Vertex; 3], base_color: [u8; 3]) {
    let mut clip = [Vec4::ZERO; 3];
    for i in 0..3 {
        let p = verts[i].pos();
        clip[i] = *mvp * vec4(p[0] as f32, p[1] as f32, p[2] as f32, 1.0);
    }
    // Reject rather than clip triangles that cross the near plane.
    if clip.iter().any(|c| c.w <= 0.001) {
        return;
    }

    let mut ndc_z = [0.0f32; 3];
    let mut screen = [(0.0f32, 0.0f32); 3];
    for i in 0..3 {
        let inv_w = 1.0 / clip[i].w;
        ndc_z[i] = clip[i].z * inv_w;
        screen[i] = (
            (clip[i].x * inv_w * 0.5 + 0.5) * fb.width as f32,
            (1.0 - (clip[i].y * inv_w * 0.5 + 0.5)) * fb.height as f32,
        );
    }

    // Backface cull in screen space.
    let area = (screen[1].0 - screen[0].0) * (screen[2].1 - screen[0].1)
        - (screen[2].0 - screen[0].0) * (screen[1].1 - screen[0].1);
    if area >= 0.0 {
        return;
    }

    let n = verts[0].normal();
    let normal = vec3(n[0] as f32, n[1] as f32, n[2] as f32).normalize_or_zero();
    let light_dir = vec3(0.4, 0.4, 0.82).normalize();
    let lit = normal.dot(light_dir).max(0.15);
    let color = [
        (base_color[0] as f32 * lit) as u8,
        (base_color[1] as f32 * lit) as u8,
        (base_color[2] as f32 * lit) as u8,
    ];

    let min_x = screen
        .iter()
        .map(|s| s.0)
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.0) as i32;
    let max_x = screen
        .iter()
        .map(|s| s.0)
        .fold(f32::MIN, f32::max)
        .ceil()
        .min(fb.width as f32) as i32;
    let min_y = screen
        .iter()
        .map(|s| s.1)
        .fold(f32::MAX, f32::min)
        .floor()
        .max(0.0) as i32;
    let max_y = screen
        .iter()
        .map(|s| s.1)
        .fold(f32::MIN, f32::max)
        .ceil()
        .min(fb.height as f32) as i32;
    if min_x >= max_x || min_y >= max_y {
        return;
    }

    let (x0, y0) = screen[0];
    let (x1, y1) = screen[1];
    let (x2, y2) = screen[2];
    let denom = (y1 - y2) * (x0 - x2) + (x2 - x1) * (y0 - y2);
    if denom.abs() < f32::EPSILON {
        return;
    }
    for y in min_y..max_y {
        for x in min_x..max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let w0 = ((y1 - y2) * (px - x2) + (x2 - x1) * (py - y2)) / denom;
            let w1 = ((y2 - y0) * (px - x2) + (x0 - x2) * (py - y2)) / denom;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let depth = w0 * ndc_z[0] + w1 * ndc_z[1] + w2 * ndc_z[2];
            fb.set(x, y, depth, color);
        }
    }
}

/// Blit the framebuffer as half-block (`▀`) characters — foreground = top
/// pixel, background = bottom pixel — doubling effective vertical resolution.
/// Redraws in full each frame (no cell diffing yet); a single buffered write
/// keeps that from being a separate syscall per cell.
fn present(fb: &Framebuffer, cols: u16, rows: u16, fps: f32) {
    let mut out = String::with_capacity(fb.width * fb.height * 4 + 64);
    out.push_str("\x1b[H");
    let mut last_fg: Option<[u8; 3]> = None;
    let mut last_bg: Option<[u8; 3]> = None;
    for y in 0..rows as usize {
        for x in 0..cols as usize {
            let top = fb.color[(y * 2) * fb.width + x];
            let bottom_idx = (y * 2 + 1) * fb.width + x;
            let bottom = if bottom_idx < fb.color.len() {
                fb.color[bottom_idx]
            } else {
                top
            };
            if last_fg != Some(top) {
                out.push_str(&format!("\x1b[38;2;{};{};{}m", top[0], top[1], top[2]));
                last_fg = Some(top);
            }
            if last_bg != Some(bottom) {
                out.push_str(&format!("\x1b[48;2;{};{};{}m", bottom[0], bottom[1], bottom[2]));
                last_bg = Some(bottom);
            }
            out.push('\u{2580}');
        }
        out.push_str("\x1b[0m\r\n");
        last_fg = None;
        last_bg = None;
    }
    out.push_str(&format!("\x1b[0mpetrichor64 (tui) — fps: {:.0}  esc to quit", fps));
    let mut so = stdout();
    let _ = so.write_all(out.as_bytes());
    let _ = so.flush();
}

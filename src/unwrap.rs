//! UV unwrapping: lay a mesh's triangles out flat on a fresh image.
//!
//! Used by the paint overlay's NEW (a mesh gets its own image and new uvs to
//! go with it) and by the `unwrap` native for tools that build meshes. Only the
//! layout lives here; what to do with the uvs (a new model, a glb) is the
//! caller's business.
//!
//! Charts are groups of connected triangles that face the same way. Each chart
//! is projected flat and the charts are packed onto the image at one shared
//! scale, so texels are the same size everywhere on the model, and snapped to
//! whole texels so pixel art lines up. Vertices on a chart border are split:
//! the result has its own vertex list, with `from` naming the source vertex of
//! each one so normals and anything else per-vertex can be carried across.

use std::collections::HashMap;

/// How triangles are grouped into charts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Six projection directions (±x, ±y, ±z), each triangle taking the one its
    /// face points along most; connected triangles sharing a direction form a
    /// chart. Suits the boxy, mostly axis-aligned meshes the engine makes.
    Box,
    /// One chart: everything projected along the mesh's thinnest axis. Suits
    /// flat things (signs, panels, leaves); a thick mesh gets its front and
    /// back drawn over each other.
    Flat,
}

impl Mode {
    pub fn parse(s: &str) -> Option<Mode> {
        match s.to_ascii_lowercase().as_str() {
            "box" => Some(Mode::Box),
            "flat" => Some(Mode::Flat),
            _ => None,
        }
    }
}

/// The unwrapped mesh: new vertices (positions copied from the source), their
/// uvs (0..1, v down), indices into them, and the source vertex of each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Unwrapped {
    pub pos: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    pub ind: Vec<u32>,
    pub from: Vec<u32>,
    /// How many charts the triangles fell into.
    pub charts: usize,
}

/// Unwrap triangles `ind` (three per triangle, into `pos`) onto a `w` x `h`
/// texel image, leaving `pad` texels between charts.
pub fn unwrap(pos: &[[f32; 3]], ind: &[u32], mode: Mode, w: u32, h: u32, pad: u32) -> Unwrapped {
    let tris: Vec<[u32; 3]> = ind
        .chunks_exact(3)
        .filter(|t| t.iter().all(|&i| (i as usize) < pos.len()))
        .map(|t| [t[0], t[1], t[2]])
        .collect();
    if tris.is_empty() {
        return Unwrapped::default();
    }
    let p = |i: u32| glam::Vec3::from_array(pos[i as usize]);

    // a projection direction per triangle: axis 0..3 and sign
    let dirs: Vec<u8> = match mode {
        Mode::Box => tris
            .iter()
            .map(|t| {
                let n = (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]));
                let a = n.abs();
                let axis = if a.x >= a.y && a.x >= a.z {
                    0
                } else if a.y >= a.z {
                    1
                } else {
                    2
                };
                axis * 2 + (n[axis as usize] < 0.) as u8
            })
            .collect(),
        Mode::Flat => {
            let (lo, hi) = tris.iter().flatten().fold(
                (glam::Vec3::splat(f32::MAX), glam::Vec3::splat(f32::MIN)),
                |(lo, hi), &i| (lo.min(p(i)), hi.max(p(i))),
            );
            let e = hi - lo;
            // the thinnest axis; ties go to z (looking down on the mesh)
            let axis = if e.z <= e.x && e.z <= e.y {
                2
            } else if e.y <= e.x {
                1
            } else {
                0
            };
            // facing the side most of the area faces
            let lean: f32 = tris
                .iter()
                .map(|t| (p(t[1]) - p(t[0])).cross(p(t[2]) - p(t[0]))[axis as usize])
                .sum();
            vec![axis * 2 + (lean < 0.) as u8; tris.len()]
        }
    };

    let chart_of = charts(pos, &tris, &dirs);
    let chart_n = chart_of.iter().map(|&c| c + 1).max().unwrap_or(0);

    // new vertices: one per (chart, source vertex), projected flat
    let mut out = Unwrapped {
        charts: chart_n,
        ..Default::default()
    };
    let mut flat: Vec<[f32; 2]> = vec![];
    let mut vert_chart: Vec<usize> = vec![];
    let mut made: HashMap<(usize, u32), u32> = HashMap::new();
    for (ti, t) in tris.iter().enumerate() {
        let c = chart_of[ti];
        let d = dirs[ti];
        for &i in t {
            let n = *made.entry((c, i)).or_insert_with(|| {
                out.pos.push(pos[i as usize]);
                out.from.push(i);
                flat.push(project(pos[i as usize], d));
                vert_chart.push(c);
                (out.pos.len() - 1) as u32
            });
            out.ind.push(n);
        }
    }

    // each chart's extent in model units
    let mut lo = vec![[f32::MAX; 2]; chart_n];
    let mut hi = vec![[f32::MIN; 2]; chart_n];
    for (v, f) in flat.iter().enumerate() {
        let c = vert_chart[v];
        for k in 0..2 {
            lo[c][k] = lo[c][k].min(f[k]);
            hi[c][k] = hi[c][k].max(f[k]);
        }
    }
    let size: Vec<[f32; 2]> = (0..chart_n)
        .map(|c| [hi[c][0] - lo[c][0], hi[c][1] - lo[c][1]])
        .collect();

    let (scale, at) = pack(&size, w, h, pad);
    out.uv = flat
        .iter()
        .enumerate()
        .map(|(v, f)| {
            let c = vert_chart[v];
            [
                (at[c][0] + (f[0] - lo[c][0]) * scale) / w as f32,
                (at[c][1] + (f[1] - lo[c][1]) * scale) / h as f32,
            ]
        })
        .collect();
    out
}

/// Flat coordinates for a point seen along direction `d` (axis * 2 + negative),
/// oriented the way a viewer standing on that side would read the image:
/// u to their right, v downward.
fn project(p: [f32; 3], d: u8) -> [f32; 2] {
    let [x, y, z] = p;
    match d {
        0 => [y, -z],  // +x
        1 => [-y, -z], // -x
        2 => [-x, -z], // +y
        3 => [x, -z],  // -y
        4 => [x, -y],  // +z, from above
        _ => [x, y],   // -z, from below
    }
}

/// Group triangles into charts: connected (sharing an edge, by position, since
/// meshes often split vertices at hard edges) and facing the same direction.
fn charts(pos: &[[f32; 3]], tris: &[[u32; 3]], dirs: &[u8]) -> Vec<usize> {
    // the same corner, whatever vertex it's stored in
    let key = |i: u32| {
        let p = pos[i as usize];
        [
            (p[0] * 1024.).round() as i64,
            (p[1] * 1024.).round() as i64,
            (p[2] * 1024.).round() as i64,
        ]
    };
    let mut edges: HashMap<([i64; 3], [i64; 3]), Vec<usize>> = HashMap::new();
    for (ti, t) in tris.iter().enumerate() {
        for k in 0..3 {
            let (a, b) = (key(t[k]), key(t[(k + 1) % 3]));
            let e = if a < b { (a, b) } else { (b, a) };
            edges.entry(e).or_default().push(ti);
        }
    }
    let mut chart = vec![usize::MAX; tris.len()];
    let mut n = 0;
    for start in 0..tris.len() {
        if chart[start] != usize::MAX {
            continue;
        }
        chart[start] = n;
        let mut todo = vec![start];
        while let Some(ti) = todo.pop() {
            let t = tris[ti];
            for k in 0..3 {
                let (a, b) = (key(t[k]), key(t[(k + 1) % 3]));
                let e = if a < b { (a, b) } else { (b, a) };
                for &o in &edges[&e] {
                    if chart[o] == usize::MAX && dirs[o] == dirs[ti] {
                        chart[o] = n;
                        todo.push(o);
                    }
                }
            }
        }
        n += 1;
    }
    chart
}

/// Place rectangles (model units) on a `w` x `h` texel image at the largest
/// shared scale that fits, in shelves, tallest first, each on whole texels
/// with `pad` texels around it. Returns the scale (texels per model unit) and
/// each rectangle's top-left texel. If nothing fits even at a tiny scale (more
/// charts than texels), they overlap rather than fail.
fn pack(size: &[[f32; 2]], w: u32, h: u32, pad: u32) -> (f32, Vec<[f32; 2]>) {
    let mut order: Vec<usize> = (0..size.len()).collect();
    order.sort_by(|&a, &b| size[b][1].total_cmp(&size[a][1]).then(size[b][0].total_cmp(&size[a][0])));

    let place = |scale: f32, pad: u32| -> Option<Vec<[f32; 2]>> {
        let mut at = vec![[0f32; 2]; size.len()];
        let (mut x, mut y, mut row) = (0u32, 0u32, 0u32);
        for &i in &order {
            let cw = (size[i][0] * scale).ceil().max(1.) as u32 + pad * 2;
            let ch = (size[i][1] * scale).ceil().max(1.) as u32 + pad * 2;
            if cw > w {
                return None;
            }
            if x + cw > w {
                x = 0;
                y += row;
                row = 0;
            }
            if y + ch > h {
                return None;
            }
            at[i] = [(x + pad) as f32, (y + pad) as f32];
            x += cw;
            row = row.max(ch);
        }
        Some(at)
    };

    let biggest = size.iter().fold(0f32, |m, s| m.max(s[0]).max(s[1]));
    if biggest <= 0. {
        return (0., vec![[0., 0.]; size.len()]);
    }
    for pad in [pad, 0] {
        // whole charts are at least a texel, so a tiny scale either fits or never will
        let mut lo = 0f32;
        let mut hi = w.max(h) as f32 / biggest;
        if place(lo.max(1e-6), pad).is_none() {
            continue;
        }
        for _ in 0..40 {
            let mid = (lo + hi) * 0.5;
            if place(mid, pad).is_some() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
        // prefer a scale that makes the biggest chart a whole number of texels
        // across, so its texels line up with the mesh's own edges
        let round = (lo * biggest).round() / biggest;
        let scale = if round > lo && place(round, pad).is_some() { round } else { lo };
        if let Some(at) = place(scale, pad) {
            return (scale, at);
        }
    }
    let s = (w.min(h) as f32 / biggest).max(0.);
    (s, vec![[0., 0.]; size.len()])
}

/// Paint a `w` x `h` image the way the unwrapped mesh looked before: every
/// texel a triangle covers takes `sample` at the matching point of the old
/// mapping (`old`, one per unwrapped vertex, in whatever space `sample` reads).
/// A triangle too thin to cover a texel centre still gets the texel under its
/// middle, and chart edges are grown a couple of texels into the padding so
/// nearest-texel sampling never shows the background along a seam.
pub fn bake(
    un: &Unwrapped,
    old: &[[f32; 2]],
    w: u32,
    h: u32,
    sample: impl Fn([f32; 2]) -> [u8; 4],
) -> image::RgbaImage {
    let mut img = image::RgbaImage::new(w, h);
    let mut filled = vec![false; (w * h) as usize];
    let mut put = |x: u32, y: u32, c: [u8; 4], img: &mut image::RgbaImage| {
        img.put_pixel(x, y, image::Rgba(c));
        filled[(y * w + x) as usize] = true;
    };
    let lerp = |t: &[u32], b: [f32; 3]| -> [f32; 2] {
        let (a, bb, c) = (old[t[0] as usize], old[t[1] as usize], old[t[2] as usize]);
        [
            a[0] * b[0] + bb[0] * b[1] + c[0] * b[2],
            a[1] * b[0] + bb[1] * b[1] + c[1] * b[2],
        ]
    };
    for t in un.ind.chunks_exact(3) {
        let px = |i: u32| {
            let u = un.uv[i as usize];
            [u[0] * w as f32, u[1] * h as f32]
        };
        let (a, b, c) = (px(t[0]), px(t[1]), px(t[2]));
        let area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
        let mid = [(a[0] + b[0] + c[0]) / 3., (a[1] + b[1] + c[1]) / 3.];
        let (mx, my) = (mid[0].floor().max(0.) as u32, mid[1].floor().max(0.) as u32);
        if mx < w && my < h {
            put(mx, my, sample(lerp(t, [1. / 3.; 3])), &mut img);
        }
        if area.abs() < 1e-9 {
            continue;
        }
        let x0 = a[0].min(b[0]).min(c[0]).floor().max(0.) as u32;
        let y0 = a[1].min(b[1]).min(c[1]).floor().max(0.) as u32;
        let x1 = (a[0].max(b[0]).max(c[0]).ceil() as u32).min(w);
        let y1 = (a[1].max(b[1]).max(c[1]).ceil() as u32).min(h);
        for y in y0..y1 {
            for x in x0..x1 {
                let p = [x as f32 + 0.5, y as f32 + 0.5];
                let wa = ((b[0] - p[0]) * (c[1] - p[1]) - (b[1] - p[1]) * (c[0] - p[0])) / area;
                let wb = ((c[0] - p[0]) * (a[1] - p[1]) - (c[1] - p[1]) * (a[0] - p[0])) / area;
                let wc = 1. - wa - wb;
                if wa >= -1e-4 && wb >= -1e-4 && wc >= -1e-4 {
                    put(x, y, sample(lerp(t, [wa, wb, wc])), &mut img);
                }
            }
        }
    }
    for _ in 0..2 {
        let before = filled.clone();
        for y in 0..h {
            for x in 0..w {
                if before[(y * w + x) as usize] {
                    continue;
                }
                let near = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().find_map(|(dx, dy)| {
                    let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                    let inside = nx >= 0 && ny >= 0 && (nx as u32) < w && (ny as u32) < h;
                    (inside && before[(ny as u32 * w + nx as u32) as usize])
                        .then(|| *img.get_pixel(nx as u32, ny as u32))
                });
                if let Some(c) = near {
                    img.put_pixel(x, y, c);
                    filled[(y * w + x) as usize] = true;
                }
            }
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A unit cube, 24 vertices (split per face) like the engine's own.
    fn cube() -> (Vec<[f32; 3]>, Vec<u32>) {
        let mut pos = vec![];
        let mut ind = vec![];
        let faces: [[[f32; 3]; 4]; 6] = [
            [[1., 0., 0.], [1., 1., 0.], [1., 1., 1.], [1., 0., 1.]],
            [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
            [[0., 1., 0.], [0., 1., 1.], [1., 1., 1.], [1., 1., 0.]],
            [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
            [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
            [[0., 0., 0.], [0., 1., 0.], [1., 1., 0.], [1., 0., 0.]],
        ];
        for f in faces {
            let b = pos.len() as u32;
            pos.extend_from_slice(&f);
            ind.extend_from_slice(&[b, b + 1, b + 2, b, b + 2, b + 3]);
        }
        (pos, ind)
    }

    #[test]
    fn a_cube_unwraps_to_six_separate_squares() {
        let (pos, ind) = cube();
        let u = unwrap(&pos, &ind, Mode::Box, 32, 32, 1);
        assert_eq!(u.charts, 6);
        assert_eq!(u.ind.len(), ind.len());
        assert_eq!(u.pos.len(), u.uv.len());
        assert!(u.uv.iter().all(|t| (0. ..=1.).contains(&t[0]) && (0. ..=1.).contains(&t[1])));
        // every face is a square of the same size (one shared scale)
        let mut rects = vec![];
        for t in u.ind.chunks_exact(6) {
            let us: Vec<[f32; 2]> = t.iter().map(|&i| u.uv[i as usize]).collect();
            let lo = us.iter().fold([f32::MAX; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
            let hi = us.iter().fold([f32::MIN; 2], |m, p| [m[0].max(p[0]), m[1].max(p[1])]);
            rects.push([lo[0] * 32., lo[1] * 32., hi[0] * 32., hi[1] * 32.]);
        }
        let side = rects[0][2] - rects[0][0];
        assert!(side >= 8., "faces use the space: {}", side);
        for r in &rects {
            assert!(((r[2] - r[0]) - side).abs() < 1e-3 && ((r[3] - r[1]) - side).abs() < 1e-3);
        }
        // and none overlap
        for (i, a) in rects.iter().enumerate() {
            for b in rects.iter().skip(i + 1) {
                let apart = a[2] <= b[0] || b[2] <= a[0] || a[3] <= b[1] || b[3] <= a[1];
                assert!(apart, "{:?} overlaps {:?}", a, b);
            }
        }
    }

    #[test]
    fn charts_start_on_whole_texels() {
        let (pos, ind) = cube();
        let u = unwrap(&pos, &ind, Mode::Box, 16, 16, 1);
        for t in u.ind.chunks_exact(6) {
            let lo = t
                .iter()
                .map(|&i| u.uv[i as usize])
                .fold([f32::MAX; 2], |m, p| [m[0].min(p[0]), m[1].min(p[1])]);
            for k in 0..2 {
                let px = lo[k] * 16.;
                assert!((px - px.round()).abs() < 1e-3, "chart edge at {}", px);
            }
        }
    }

    #[test]
    fn source_vertices_are_carried_across() {
        let (pos, ind) = cube();
        let u = unwrap(&pos, &ind, Mode::Box, 16, 16, 1);
        for (v, &f) in u.from.iter().enumerate() {
            assert_eq!(u.pos[v], pos[f as usize]);
        }
        // the same triangles, in the same order
        for (a, b) in u.ind.iter().zip(ind.iter()) {
            assert_eq!(u.from[*a as usize], *b);
        }
    }

    #[test]
    fn flat_puts_a_quad_on_the_whole_image() {
        let pos = vec![[0., 0., 0.], [2., 0., 0.], [2., 1., 0.], [0., 1., 0.]];
        let ind = vec![0, 1, 2, 0, 2, 3];
        let u = unwrap(&pos, &ind, Mode::Flat, 16, 16, 0);
        assert_eq!(u.charts, 1);
        let wide = u.uv.iter().fold(0f32, |m, p| m.max(p[0]));
        assert!(wide > 0.99, "fills the width: {}", wide);
    }

    #[test]
    fn a_shared_edge_joins_triangles_split_at_their_vertices() {
        // two triangles of one face, no shared vertex indices
        let pos = vec![
            [0., 0., 0.], [1., 0., 0.], [1., 1., 0.],
            [0., 0., 0.], [1., 1., 0.], [0., 1., 0.],
        ];
        let ind = vec![0, 1, 2, 3, 4, 5];
        let u = unwrap(&pos, &ind, Mode::Box, 16, 16, 1);
        assert_eq!(u.charts, 1);
    }

    #[test]
    fn baking_carries_the_old_look_onto_the_new_layout() {
        let (pos, ind) = cube();
        let u = unwrap(&pos, &ind, Mode::Box, 16, 16, 1);
        // the old look: red where the source vertex had x = 1, blue elsewhere
        let old: Vec<[f32; 2]> = u.from.iter().map(|&f| [pos[f as usize][0], 0.]).collect();
        let img = bake(&u, &old, 16, 16, |p| if p[0] > 0.5 { [255, 0, 0, 255] } else { [0, 0, 255, 255] });
        // the +x face is solid red: find its chart's middle
        let face: Vec<[f32; 2]> = u.ind[0..6].iter().map(|&i| u.uv[i as usize]).collect();
        let cx = face.iter().map(|p| p[0]).sum::<f32>() / 6. * 16.;
        let cy = face.iter().map(|p| p[1]).sum::<f32>() / 6. * 16.;
        assert_eq!(img.get_pixel(cx as u32, cy as u32).0, [255, 0, 0, 255]);
        let opaque = img.pixels().filter(|p| p.0[3] > 0).count();
        assert!(opaque > 6 * 9, "the faces are painted: {}", opaque);
    }
}

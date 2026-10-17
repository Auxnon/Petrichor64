//! Cursor picking against what is actually on screen, for overlays.
//!
//! `app.pick()` answers "what is under the pointer, and which texel of which
//! texture is it showing?" — the question an on-the-fly texture editor needs.
//! Rather than approximating with colliders, it ray-casts the same data the
//! renderer draws: each entity's model triangles through its current instance
//! matrix (`EntityUniforms::model`), billboard sprites as the camera-facing
//! quad `vs_common` builds, and the bundle's tile chunks. The interpolated
//! vertex uv goes through the entity's `uv_mod`, exactly as the vertex shader
//! does, which lands on the atlas; the atlas uv names the texture and the texel.
//!
//! Everything here is in the engine's internal units (world × 16) until the
//! result is handed back, when positions and distances are divided back out.

use glam::{vec2, vec3, vec4, Mat4, Vec2, Vec3, Vec4};

use crate::root::Core;

/// What the ray hit.
#[derive(Clone, Debug)]
pub enum PickKind {
    Ent { id: u64, asset: String },
    Tile { x: i32, y: i32, z: i32 },
}

/// One hit, in world units, plus the texture under it if the atlas uv falls
/// inside a named texture.
#[derive(Clone, Debug)]
pub struct PickHit {
    pub kind: PickKind,
    pub dist: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub tex: Option<PickTex>,
}

#[derive(Clone, Debug)]
pub struct PickTex {
    pub name: String,
    /// Texel under the ray, and the texture's size in pixels.
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
    pub rgba: [u8; 4],
}

/// A ray hit before the texture lookup: `t` along a normalized internal-unit ray.
struct RawHit {
    t: f32,
    normal: Vec3,
    atlas_uv: Vec2,
    kind: PickKind,
}

/// The pointer ray for the camera the last frame drew with, as
/// `(origin, direction)` in internal units. `None` before the first frame.
pub fn cursor_ray(core: &Core) -> Option<(Vec3, Vec3)> {
    let m = core.global.mouse_pos;
    screen_ray(core, m.x, m.y)
}

/// The ray through screen point `(x, y)`, each 0..1 from the top left.
pub fn screen_ray(core: &Core, x: f32, y: f32) -> Option<(Vec3, Vec3)> {
    let (persp, view) = core.global.last_cam_matrices?;
    let inv = (persp * view).inverse();
    let nx = 2. * x - 1.;
    let ny = 1. - 2. * y;
    // wgpu's clip depth is 0..1, near to far.
    let near = inv.project_point3(vec3(nx, ny, 0.));
    let far = inv.project_point3(vec3(nx, ny, 0.5));
    let dir = (far - near).normalize_or_zero();
    if dir == Vec3::ZERO {
        return None;
    }
    Some((near, dir))
}

/// Pick against `bundle`'s entities, and its tiles too when `tiles` is set.
/// Hits on fully transparent texels are skipped, as the fragment shader
/// discards them; the ray carries on to whatever is behind.
pub fn pick(core: &Core, bundle: u8, tiles: bool, ray: (Vec3, Vec3)) -> Option<PickHit> {
    let (origin, dir) = ray;
    let view = core.global.last_cam_matrices.map(|m| m.1)?;
    let mut hits: Vec<RawHit> = Vec::new();

    if let Some(b) = core.ent_manager.bundles.get(&bundle) {
        for (eref, ent, uni) in b.array.iter() {
            let (id, asset, dead) = match eref.with_ref(|l| Ok((l.get_id(), l.get_asset(), l.is_dead()))) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if dead {
                continue;
            }
            let u = uni.borrow();
            let model_mat = Mat4::from_cols_array_2d(&u.model);
            let uv_mod = Vec4::from_array(u.uv_mod);
            let kind = PickKind::Ent { id, asset };
            if u.effects[0] > 0. {
                if let Some(h) = billboard_hit(origin, dir, view, model_mat, u.effects[0], u.effects[1], uv_mod) {
                    hits.push(RawHit { kind, ..h });
                }
                continue;
            }
            let Some((verts, inds)) = ent.model.data.as_ref() else {
                continue;
            };
            let bounds = (ent.model.bounds_min * 16., ent.model.bounds_max * 16.);
            mesh_hits(origin, dir, model_mat, verts, inds, Some(bounds), uv_mod, &kind, &mut hits);
        }
    }

    if tiles {
        for c in core.world.chunk_models_for(bundle) {
            if c.ind_data.is_empty() {
                continue;
            }
            let lo = c.pos.as_vec3() * 16.;
            // Dual-grid tileset meshes reach a little past their chunk; pad by a cell.
            let pad = Vec3::splat(16.);
            let size = Vec3::splat(crate::tile::CHUNK_SIZE as f32 * 16.);
            let model_mat = Mat4::from_translation(lo);
            let kind = PickKind::Tile { x: 0, y: 0, z: 0 };
            let start = hits.len();
            mesh_hits(
                origin,
                dir,
                model_mat,
                &c.vert_data,
                &c.ind_data,
                Some((-pad, size + pad)),
                vec4(0., 0., 1., 1.),
                &kind,
                &mut hits,
            );
            // The cell is the one the hit point sits in, nudged back along the
            // face normal so a hit exactly on a face resolves to its own cube.
            // Cubes are centred on their integer coordinates, hence the round.
            for h in hits[start..].iter_mut() {
                let p = (origin + dir * h.t) / 16. - h.normal * 0.01;
                let cell = (p + Vec3::splat(0.5)).floor();
                h.kind = PickKind::Tile {
                    x: cell.x as i32,
                    y: cell.y as i32,
                    z: cell.z as i32,
                };
            }
        }
    }

    hits.sort_by(|a, b| a.t.total_cmp(&b.t));
    for h in hits {
        let tex = texel_at(core, h.atlas_uv);
        if let Some(t) = &tex {
            if t.rgba[3] < 26 {
                continue; // transparent: the shader discards it, so look behind
            }
        }
        return Some(PickHit {
            kind: h.kind,
            dist: h.t / 16.,
            point: (origin + dir * h.t) / 16.,
            normal: h.normal,
            tex,
        });
    }
    None
}

/// Every triangle hit of one mesh, appended to `out`. The ray is taken into
/// model space, so the AABB check and the triangle tests both run on the
/// mesh's own packed coordinates; `t` stays comparable because the local
/// direction isn't renormalized.
#[allow(clippy::too_many_arguments)]
fn mesh_hits(
    origin: Vec3,
    dir: Vec3,
    model_mat: Mat4,
    verts: &[crate::model::Vertex],
    inds: &[u32],
    bounds: Option<(Vec3, Vec3)>,
    uv_mod: Vec4,
    kind: &PickKind,
    out: &mut Vec<RawHit>,
) {
    let det = model_mat.determinant();
    if det.abs() < 1e-12 {
        return;
    }
    let inv = model_mat.inverse();
    let lo = inv.transform_point3(origin);
    let ld = inv.transform_vector3(dir);
    if let Some((bmin, bmax)) = bounds {
        if bmin != bmax && !ray_aabb(lo, ld, bmin - Vec3::splat(0.5), bmax + Vec3::splat(0.5)) {
            return;
        }
    }
    let normal_mat = model_mat.inverse().transpose();
    for tri in inds.chunks_exact(3) {
        let (Some(a), Some(b), Some(c)) = (
            verts.get(tri[0] as usize),
            verts.get(tri[1] as usize),
            verts.get(tri[2] as usize),
        ) else {
            continue;
        };
        let pa = pos3(a);
        let pb = pos3(b);
        let pc = pos3(c);
        // Two-sided: the engine doesn't cull, so a face seen from behind is still drawn.
        if let Some((t, u, v)) = ray_tri(lo, ld, pa, pb, pc) {
            let w = 1. - u - v;
            let ta = Vec2::from_array(a.tex());
            let tb = Vec2::from_array(b.tex());
            let tc = Vec2::from_array(c.tex());
            let uv = ta * w + tb * u + tc * v;
            let atlas_uv = uv * vec2(uv_mod.z, uv_mod.w) + vec2(uv_mod.x, uv_mod.y);
            let n_local = (pb - pa).cross(pc - pa);
            let mut normal = normal_mat.transform_vector3(n_local).normalize_or_zero();
            if normal.dot(dir) > 0. {
                normal = -normal;
            }
            out.push(RawHit {
                t,
                normal,
                atlas_uv,
                kind: kind.clone(),
            });
        }
    }
}

/// A billboard sprite, as `vs_common` draws it: the quad sits in view space at
/// the entity's centre, scaled by `bb` (effects.x × scale) and spun by `r`
/// (effects.y), always facing the camera. Uses the `PLANE` model's layout:
/// corners at ±8, u along +x, v down from +y.
fn billboard_hit(
    origin: Vec3,
    dir: Vec3,
    view: Mat4,
    model_mat: Mat4,
    bb: f32,
    r: f32,
    uv_mod: Vec4,
) -> Option<RawHit> {
    let c = view * model_mat * vec4(0., 0., 0., 1.);
    let o = view.transform_point3(origin);
    let d = view.transform_vector3(dir);
    if d.z.abs() < 1e-6 {
        return None;
    }
    let t = (c.z - o.z) / d.z;
    if t <= 0. {
        return None;
    }
    let p = o + d * t;
    let ox = (p.x - c.x) / bb;
    let oy = (p.y - c.y) / bb;
    // Undo the shader's spin: it maps (px, py) to (px·cos + py·sin, −px·sin + py·cos).
    let (s, co) = r.sin_cos();
    let px = co * ox - s * oy;
    let py = s * ox + co * oy;
    if px.abs() > 8. || py.abs() > 8. {
        return None;
    }
    let uv = vec2((px / 8. + 1.) * 0.5, (1. - py / 8.) * 0.5);
    let atlas_uv = uv * vec2(uv_mod.z, uv_mod.w) + vec2(uv_mod.x, uv_mod.y);
    Some(RawHit {
        t,
        normal: -dir,
        atlas_uv,
        kind: PickKind::Tile { x: 0, y: 0, z: 0 }, // replaced by the caller
    })
}

/// The named texture containing an atlas uv, and the texel there. When rects
/// overlap the smallest wins — the most specific name for what's drawn.
pub fn texel_at(core: &Core, uv: Vec2) -> Option<PickTex> {
    let tm = &core.tex_manager;
    let aw = tm.atlas_dim.x as f32;
    let ah = tm.atlas_dim.y as f32;
    let mut best: Option<(&String, &Vec4)> = None;
    for (name, r) in tm.dictionary.iter() {
        if r.z <= 0. || r.w <= 0. {
            continue;
        }
        if uv.x >= r.x && uv.y >= r.y && uv.x < r.x + r.z && uv.y < r.y + r.w {
            let better = match best {
                Some((_, b)) => r.z * r.w < b.z * b.w,
                None => true,
            };
            if better {
                best = Some((name, r));
            }
        }
    }
    let (name, r) = best?;
    let w = (r.z * aw).round().max(1.) as u32;
    let h = (r.w * ah).round().max(1.) as u32;
    let x = (((uv.x - r.x) / r.z) * w as f32).floor().clamp(0., (w - 1) as f32) as u32;
    let y = (((uv.y - r.y) / r.w) * h as f32).floor().clamp(0., (h - 1) as f32) as u32;
    let ax = ((r.x * aw).round() as u32 + x).min(tm.atlas_dim.x - 1);
    let ay = ((r.y * ah).round() as u32 + y).min(tm.atlas_dim.y - 1);
    let rgba = tm.atlas.get_pixel(ax, ay).0;
    Some(PickTex {
        name: name.clone(),
        x,
        y,
        w,
        h,
        rgba,
    })
}

/// The most common colours of a named texture, most frequent first, ignoring
/// transparent texels — a best guess at the palette it was painted with. Ties
/// keep the order the colours first appear in (top-left, row by row), which
/// for a palette texture of equal swatches is its own layout.
pub fn texture_colors(core: &Core, name: &str, max: usize) -> Vec<[u8; 4]> {
    let tm = &core.tex_manager;
    let Some(r) = tm.dictionary.get(name) else {
        return vec![];
    };
    let aw = tm.atlas_dim.x as f32;
    let ah = tm.atlas_dim.y as f32;
    let x0 = (r.x * aw).round() as u32;
    let y0 = (r.y * ah).round() as u32;
    let w = (r.z * aw).round() as u32;
    let h = (r.w * ah).round() as u32;
    // colour -> (count, first seen)
    let mut counts: std::collections::HashMap<[u8; 4], (u32, u32)> = Default::default();
    let mut seen = 0u32;
    for y in y0..(y0 + h).min(tm.atlas_dim.y) {
        for x in x0..(x0 + w).min(tm.atlas_dim.x) {
            let p = tm.atlas.get_pixel(x, y).0;
            if p[3] < 26 {
                continue;
            }
            let e = counts.entry(p).or_insert((0, seen));
            e.0 += 1;
            seen += 1;
        }
    }
    let mut v: Vec<([u8; 4], (u32, u32))> = counts.into_iter().collect();
    v.sort_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.1 .1.cmp(&b.1 .1)));
    v.into_iter().take(max).map(|(c, _)| c).collect()
}

/// Project a world-unit point to the screen (0..1 from the top left) and its
/// view depth in world units. `None` when it's behind the camera.
pub fn project(core: &Core, p: Vec3) -> Option<(f32, f32, f32)> {
    let (persp, view) = core.global.last_cam_matrices?;
    let v = view * (p * 16.).extend(1.);
    if v.z >= 0. {
        return None; // right-handed view: in front is −z
    }
    let c = persp * v;
    let x = (c.x / c.w + 1.) * 0.5;
    let y = (1. - c.y / c.w) * 0.5;
    Some((x, y, -v.z / 16.))
}

fn pos3(v: &crate::model::Vertex) -> Vec3 {
    let p = v.pos();
    vec3(p[0] as f32, p[1] as f32, p[2] as f32)
}

/// Möller–Trumbore, two-sided: `(t, u, v)` with barycentrics for b and c.
fn ray_tri(o: Vec3, d: Vec3, a: Vec3, b: Vec3, c: Vec3) -> Option<(f32, f32, f32)> {
    let e1 = b - a;
    let e2 = c - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-9 {
        return None;
    }
    let inv = 1. / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0. ..=1.).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0. || u + v > 1. {
        return None;
    }
    let t = e2.dot(q) * inv;
    if t > 1e-4 {
        Some((t, u, v))
    } else {
        None
    }
}

fn ray_aabb(o: Vec3, d: Vec3, lo: Vec3, hi: Vec3) -> bool {
    let mut tmin = f32::NEG_INFINITY;
    let mut tmax = f32::INFINITY;
    for i in 0..3 {
        if d[i].abs() < 1e-12 {
            if o[i] < lo[i] || o[i] > hi[i] {
                return false;
            }
        } else {
            let t1 = (lo[i] - o[i]) / d[i];
            let t2 = (hi[i] - o[i]) / d[i];
            tmin = tmin.max(t1.min(t2));
            tmax = tmax.min(t1.max(t2));
        }
    }
    tmax >= tmin.max(0.)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ray_hits_a_triangle_and_reports_barycentrics() {
        let o = vec3(0.25, 0.25, 5.);
        let d = vec3(0., 0., -1.);
        let (t, u, v) = ray_tri(o, d, vec3(0., 0., 0.), vec3(1., 0., 0.), vec3(0., 1., 0.)).unwrap();
        assert!((t - 5.).abs() < 1e-5);
        assert!((u - 0.25).abs() < 1e-5 && (v - 0.25).abs() < 1e-5);
        // and from behind: the engine draws both sides, so picking does too
        let back = ray_tri(vec3(0.25, 0.25, -5.), vec3(0., 0., 1.), vec3(0., 0., 0.), vec3(1., 0., 0.), vec3(0., 1., 0.));
        assert!(back.is_some());
    }

    #[test]
    fn ray_misses_outside_the_triangle_and_behind_the_origin() {
        let tri = (vec3(0., 0., 0.), vec3(1., 0., 0.), vec3(0., 1., 0.));
        assert!(ray_tri(vec3(0.9, 0.9, 5.), vec3(0., 0., -1.), tri.0, tri.1, tri.2).is_none());
        assert!(ray_tri(vec3(0.2, 0.2, -5.), vec3(0., 0., -1.), tri.0, tri.1, tri.2).is_none());
    }

    #[test]
    fn aabb_slab_test() {
        let lo = Vec3::splat(-1.);
        let hi = Vec3::splat(1.);
        assert!(ray_aabb(vec3(0., 0., 5.), vec3(0., 0., -1.), lo, hi));
        assert!(!ray_aabb(vec3(3., 0., 5.), vec3(0., 0., -1.), lo, hi));
        assert!(!ray_aabb(vec3(0., 0., 5.), vec3(0., 0., 1.), lo, hi), "pointing away");
    }

    #[test]
    fn billboard_centre_maps_to_the_middle_of_the_texture() {
        // camera at the origin looking down −z; sprite 10 units ahead
        let view = Mat4::IDENTITY;
        let model = Mat4::from_translation(vec3(0., 0., -10.));
        let h = billboard_hit(Vec3::ZERO, vec3(0., 0., -1.), view, model, 1., 0., vec4(0., 0., 1., 1.)).unwrap();
        assert!((h.atlas_uv - vec2(0.5, 0.5)).length() < 1e-5);
        assert!((h.t - 10.).abs() < 1e-5);
        // upper-left corner region: −x is u=0, +y is v=0
        let d = vec3(-7.9, 7.9, -10.).normalize();
        let h = billboard_hit(Vec3::ZERO, d, view, model, 1., 0., vec4(0., 0., 1., 1.)).unwrap();
        assert!(h.atlas_uv.x < 0.02 && h.atlas_uv.y < 0.02);
        // and the uv_mod puts it into the sprite's atlas rect
        let h = billboard_hit(Vec3::ZERO, vec3(0., 0., -1.), view, model, 1., 0., vec4(0.5, 0.25, 0.1, 0.1)).unwrap();
        assert!((h.atlas_uv - vec2(0.55, 0.30)).length() < 1e-5);
    }
}

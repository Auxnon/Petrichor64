//! Collision shapes and pairwise normal/penetration math for the `hit.*` /
//! `ent:hit` Lua API — see `guide/hit.md`. Pure, allocation-free functions so
//! they're cheap to call from the O(n²)-avoided broad phase and easy to unit
//! test in isolation.
//!
//! No rotation is modeled: every collider is axis-aligned in world space,
//! matching the "AABB + swept AABB" shape agreed in HANDOFF.md. World-space
//! units here match every other world-space quantity in the engine (entity
//! `pos * 16`, tile index * 16 — see `ent::render_transform` and `tile.rs`'s
//! "index position would be position divided by 16").

use glam::{Mat4, Vec3};

/// One tile, in world-space units — a solid tile at integer index `(ix,iy,iz)`
/// occupies `[ix*16, (ix+1)*16]` per axis (`tile.rs`: "index position would be
/// position divided by 16").
pub const TILE_SIZE: f32 = 16.;

/// An axis-aligned collider in world space.
#[derive(Clone, Copy, Debug)]
pub enum Collider {
    /// Axis-aligned box: `half` is the half-extent on each axis.
    Box { center: Vec3, half: Vec3 },
    /// Z-axis-aligned cylinder: `radius` in XY, `half_height` along Z.
    Cyl {
        center: Vec3,
        radius: f32,
        half_height: f32,
    },
}

/// A collision result: `normal` is the unit direction to move the *first*
/// collider (the `a` argument to `test`) to separate it from the second —
/// i.e. push `a` by `normal * depth` to (just) clear the overlap. `depth` is
/// always > 0 for a real hit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    pub normal: Vec3,
    pub depth: f32,
}

impl Collider {
    pub fn center(&self) -> Vec3 {
        match self {
            Collider::Box { center, .. } => *center,
            Collider::Cyl { center, .. } => *center,
        }
    }

    /// World-space axis-aligned bounding box `(min, max)` of this collider —
    /// used to enumerate candidate tiles for `EntManager::hit_cell`.
    pub fn aabb(&self) -> (Vec3, Vec3) {
        match self {
            Collider::Box { center, half } => (*center - *half, *center + *half),
            Collider::Cyl {
                center,
                radius,
                half_height,
            } => {
                let r = Vec3::new(*radius, *radius, *half_height);
                (*center - r, *center + r)
            }
        }
    }
}

/// The world-space box for a solid tile at integer index `(ix, iy, iz)`.
pub fn tile_box(ix: i32, iy: i32, iz: i32) -> Collider {
    Collider::Box {
        center: Vec3::new(ix as f32 + 0.5, iy as f32 + 0.5, iz as f32 + 0.5) * TILE_SIZE,
        half: Vec3::splat(TILE_SIZE * 0.5),
    }
}

/// Test any two colliders. Order matters only for `normal`'s direction (it
/// always points away from `b`, toward where `a` should move).
pub fn test(a: &Collider, b: &Collider) -> Option<Hit> {
    match (a, b) {
        (Collider::Box { center: ac, half: ah }, Collider::Box { center: bc, half: bh }) => {
            box_box(*ac, *ah, *bc, *bh)
        }
        (
            Collider::Cyl {
                center: ac,
                radius: ar,
                half_height: ahh,
            },
            Collider::Cyl {
                center: bc,
                radius: br,
                half_height: bhh,
            },
        ) => cyl_cyl(*ac, *ar, *ahh, *bc, *br, *bhh),
        (Collider::Cyl { center: ac, radius: ar, half_height: ahh }, Collider::Box { center: bc, half: bh }) => {
            cyl_box(*ac, *ar, *ahh, *bc, *bh)
        }
        (Collider::Box { center: ac, half: ah }, Collider::Cyl { center: bc, radius: br, half_height: bhh }) => {
            // Reuse cyl_box with the arguments swapped, then flip the normal
            // back so it still points away from `b` (the box here).
            cyl_box(*bc, *br, *bhh, *ac, *ah).map(|h| Hit {
                normal: -h.normal,
                depth: h.depth,
            })
        }
    }
}

/// Standard axis-of-least-overlap MTV (minimum translation vector). The axis
/// with the smallest overlap is the separating axis — this is the normal "at
/// the collision point" the way every simple 3D engine computes it for box
/// pairs, and handles the "push away along a normal" case at a corner better
/// than a naive center-to-center vector would.
fn box_box(ac: Vec3, ah: Vec3, bc: Vec3, bh: Vec3) -> Option<Hit> {
    let d = bc - ac;
    let overlap = ah + bh - d.abs();
    if overlap.x <= 0. || overlap.y <= 0. || overlap.z <= 0. {
        return None;
    }
    let (axis, depth) = if overlap.x <= overlap.y && overlap.x <= overlap.z {
        (Vec3::X, overlap.x)
    } else if overlap.y <= overlap.z {
        (Vec3::Y, overlap.y)
    } else {
        (Vec3::Z, overlap.z)
    };
    // b is on the +axis side of a when d.dot(axis) > 0 — push a the other way.
    let sign = if d.dot(axis) > 0. { -1. } else { 1. };
    Some(Hit {
        normal: axis * sign,
        depth,
    })
}

/// Radial XY push, gated by Z-range overlap — a standing character pushed
/// sideways away from another, never up/down.
fn cyl_cyl(ac: Vec3, ar: f32, ahh: f32, bc: Vec3, br: f32, bhh: f32) -> Option<Hit> {
    if ac.z - ahh >= bc.z + bhh || bc.z - bhh >= ac.z + ahh {
        return None;
    }
    let dx = ac.x - bc.x;
    let dy = ac.y - bc.y;
    let dist2 = dx * dx + dy * dy;
    let combined = ar + br;
    if dist2 >= combined * combined {
        return None;
    }
    let dist = dist2.sqrt();
    let normal = if dist > 1e-5 {
        Vec3::new(dx / dist, dy / dist, 0.)
    } else {
        // Same center: arbitrary but deterministic push direction.
        Vec3::X
    };
    Some(Hit {
        normal,
        depth: combined - dist,
    })
}

/// Closest-point-on-box-to-cylinder-center normal — the one place a naive
/// center-to-center vector would be wrong (hitting a box corner), so this
/// gives the true contact normal there instead.
fn cyl_box(ac: Vec3, ar: f32, ahh: f32, bc: Vec3, bh: Vec3) -> Option<Hit> {
    if ac.z - ahh >= bc.z + bh.z || bc.z - bh.z >= ac.z + ahh {
        return None;
    }
    let closest_x = ac.x.clamp(bc.x - bh.x, bc.x + bh.x);
    let closest_y = ac.y.clamp(bc.y - bh.y, bc.y + bh.y);
    let dx = ac.x - closest_x;
    let dy = ac.y - closest_y;
    let dist2 = dx * dx + dy * dy;
    if dist2 >= ar * ar {
        return None;
    }
    if dist2 > 1e-8 {
        let dist = dist2.sqrt();
        Some(Hit {
            normal: Vec3::new(dx / dist, dy / dist, 0.),
            depth: ar - dist,
        })
    } else {
        // Cylinder center is inside the box's XY footprint: push out along
        // whichever box edge is nearest, XY only (least-penetration axis,
        // same idea as box_box but restricted to the plane the cylinder cares
        // about).
        let pen_x = bh.x - (ac.x - bc.x).abs();
        let pen_y = bh.y - (ac.y - bc.y).abs();
        if pen_x < pen_y {
            let sign = if ac.x < bc.x { -1. } else { 1. };
            Some(Hit {
                normal: Vec3::new(sign, 0., 0.),
                depth: pen_x + ar,
            })
        } else {
            let sign = if ac.y < bc.y { -1. } else { 1. };
            Some(Hit {
                normal: Vec3::new(0., sign, 0.),
                depth: pen_y + ar,
            })
        }
    }
}

/// Test a world-space collider against an entity-owned tile grid via the
/// 3-tier cascade (entity AABB -> chunk AABB -> tile AABB) from the
/// entity-grid plan — see `guide/hit.md`. `transform` is the grid-owning
/// entity's world transform (translation * rotation only: a grid doesn't
/// inherit the owner's `size`/`scale`, its tiles stay a fixed 16-unit cube
/// like the world grid — stretching tiles non-uniformly wouldn't make sense
/// for a moving platform). Returns local grid-space tile indices plus a
/// `Hit` whose normal has been rotated back into world space.
///
/// The "local-space-transform trick": rather than computing a rotated OBB in
/// world space, `query` is moved into the grid's local space (by
/// `transform`'s inverse) and tested there with the ordinary axis-aligned
/// tests. This is exact for a `Cyl` query (its Z-axis symmetry survives any
/// rotation of the grid) and an approximation for a `Box` query under a
/// non-90°-multiple rotation (its edges no longer line up with the grid's
/// local axes) — first-pass fidelity, same caveat as the rest of this
/// module; see `guide/hit.md`.
pub fn test_grid(
    query: &Collider,
    transform: &Mat4,
    grid: &crate::tile_grid::TileGrid,
) -> Vec<(i32, i32, i32, Hit)> {
    let mut out = Vec::new();
    if grid.chunks.is_empty() {
        return out;
    }

    // Tier 1: entity AABB reject, in world space — the grid's local bounds
    // (tile-index units, scaled to world units) transformed corner-by-corner
    // since a rotation turns the box into a general parallelepiped.
    let lo = Vec3::new(
        grid.bounds_min[0] as f32,
        grid.bounds_min[1] as f32,
        grid.bounds_min[2] as f32,
    ) * TILE_SIZE;
    let hi = Vec3::new(
        grid.bounds_max[0] as f32,
        grid.bounds_max[1] as f32,
        grid.bounds_max[2] as f32,
    ) * TILE_SIZE;
    let mut world_min = Vec3::splat(f32::INFINITY);
    let mut world_max = Vec3::splat(f32::NEG_INFINITY);
    for x in [lo.x, hi.x] {
        for y in [lo.y, hi.y] {
            for z in [lo.z, hi.z] {
                let p = transform.transform_point3(Vec3::new(x, y, z));
                world_min = world_min.min(p);
                world_max = world_max.max(p);
            }
        }
    }
    let (qmin, qmax) = query.aabb();
    if qmax.x < world_min.x
        || qmin.x > world_max.x
        || qmax.y < world_min.y
        || qmin.y > world_max.y
        || qmax.z < world_min.z
        || qmin.z > world_max.z
    {
        return out;
    }

    // Move the query into the grid's local space once, up front.
    let inv = transform.inverse();
    let local_query = transform_collider(query, &inv);
    let (lqmin, lqmax) = local_query.aabb();

    let chunk_extent = crate::tile_grid::GRID_CHUNK_SIZE as f32 * TILE_SIZE;
    for chunk in grid.chunks.values() {
        // Tier 2: chunk AABB reject, in local space — cheap before touching
        // any cell data.
        let cmin = Vec3::new(chunk.pos[0] as f32, chunk.pos[1] as f32, chunk.pos[2] as f32) * TILE_SIZE;
        let cmax = cmin + Vec3::splat(chunk_extent);
        if lqmax.x < cmin.x
            || lqmin.x > cmax.x
            || lqmax.y < cmin.y
            || lqmin.y > cmax.y
            || lqmax.z < cmin.z
            || lqmin.z > cmax.z
        {
            continue;
        }

        // Tier 3: per-tile, only for chunks that survived the reject above.
        for &idx in chunk.cells.keys() {
            let (lx, ly, lz) = crate::tile_grid::unpack_local_index(idx);
            let ix = chunk.pos[0] + lx;
            let iy = chunk.pos[1] + ly;
            let iz = chunk.pos[2] + lz;
            let tile = tile_box(ix, iy, iz);
            if let Some(hit) = test(&local_query, &tile) {
                let world_normal = transform.transform_vector3(hit.normal).normalize();
                out.push((
                    ix,
                    iy,
                    iz,
                    Hit {
                        normal: world_normal,
                        depth: hit.depth,
                    },
                ));
            }
        }
    }
    out
}

fn transform_collider(c: &Collider, inv: &Mat4) -> Collider {
    match c {
        Collider::Box { center, half } => Collider::Box {
            center: inv.transform_point3(*center),
            half: *half,
        },
        Collider::Cyl {
            center,
            radius,
            half_height,
        } => Collider::Cyl {
            center: inv.transform_point3(*center),
            radius: *radius,
            half_height: *half_height,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: Vec3, b: Vec3) {
        assert!((a - b).length() < 1e-4, "{:?} != {:?}", a, b);
    }

    #[test]
    fn separated_boxes_do_not_hit() {
        let a = Collider::Box { center: Vec3::ZERO, half: Vec3::splat(1.) };
        let b = Collider::Box { center: Vec3::new(5., 0., 0.), half: Vec3::splat(1.) };
        assert_eq!(test(&a, &b), None);
    }

    #[test]
    fn overlapping_boxes_push_along_least_overlap_axis() {
        // a is a 2x2x2 box at origin, b is a 2x2x2 box overlapping mostly on X
        // (small overlap) but fully on Y/Z (large overlap) — X should win.
        let a = Collider::Box { center: Vec3::ZERO, half: Vec3::splat(1.) };
        let b = Collider::Box { center: Vec3::new(1.8, 0., 0.), half: Vec3::splat(1.) };
        let hit = test(&a, &b).expect("must overlap");
        approx(hit.normal, Vec3::new(-1., 0., 0.));
        assert!((hit.depth - 0.2).abs() < 1e-4, "depth {}", hit.depth);
    }

    #[test]
    fn box_box_normal_points_away_from_b() {
        // a sits to the left of b — pushing a "away from b" means further
        // left (-X), not toward b.
        let a = Collider::Box { center: Vec3::new(-1.8, 0., 0.), half: Vec3::splat(1.) };
        let b = Collider::Box { center: Vec3::ZERO, half: Vec3::splat(1.) };
        let hit = test(&a, &b).expect("must overlap");
        approx(hit.normal, Vec3::new(-1., 0., 0.));
    }

    #[test]
    fn cylinders_push_radially_and_ignore_far_z() {
        let a = Collider::Cyl { center: Vec3::new(0., 0., 0.), radius: 1., half_height: 1. };
        let b = Collider::Cyl { center: Vec3::new(1.5, 0., 0.), radius: 1., half_height: 1. };
        let hit = test(&a, &b).expect("must overlap");
        approx(hit.normal, Vec3::new(-1., 0., 0.));
        assert!((hit.depth - 0.5).abs() < 1e-4);

        // Same XY, but far enough apart on Z that the ranges don't overlap.
        let c = Collider::Cyl { center: Vec3::new(0., 0., 10.), radius: 1., half_height: 1. };
        assert_eq!(test(&a, &c), None);
    }

    #[test]
    fn cylinder_vs_box_corner_gives_true_contact_normal() {
        // Box at origin, half-extent 1 (spans -1..1). Cylinder centered
        // beyond the box's corner (1,1), radius large enough to reach it.
        let cyl = Collider::Cyl { center: Vec3::new(2., 2., 0.), radius: 1.6, half_height: 1. };
        let bx = Collider::Box { center: Vec3::ZERO, half: Vec3::splat(1.) };
        let hit = test(&cyl, &bx).expect("must overlap the corner");
        // Closest point is the box corner (1,1,0); normal should point
        // diagonally away from it, not straight along an axis.
        approx(hit.normal, Vec3::new(1., 1., 0.).normalize());
    }

    #[test]
    fn cyl_box_normal_direction_is_consistent_regardless_of_argument_order() {
        let cyl = Collider::Cyl { center: Vec3::new(1.5, 0., 0.), radius: 1., half_height: 1. };
        let bx = Collider::Box { center: Vec3::ZERO, half: Vec3::splat(1.) };
        let cyl_first = test(&cyl, &bx).expect("overlap");
        let box_first = test(&bx, &cyl).expect("overlap");
        // box_first's normal should point away from the cylinder, i.e. the
        // exact opposite of cyl_first's (which points away from the box).
        approx(box_first.normal, -cyl_first.normal);
        assert!((box_first.depth - cyl_first.depth).abs() < 1e-4);
    }

    #[test]
    fn test_grid_finds_a_hit_in_an_axis_aligned_grid() {
        let mut g = crate::tile_grid::TileGrid::new();
        g.set_tile("wall".to_string(), 0, 0, 0, 0); // local tile spans world [0,16]^3
        let transform = Mat4::IDENTITY;
        let query = Collider::Box {
            center: Vec3::new(8., 8., 8.),
            half: Vec3::splat(2.),
        };
        let hits = test_grid(&query, &transform, &g);
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].0, hits[0].1, hits[0].2), (0, 0, 0));
    }

    #[test]
    fn test_grid_rejects_when_the_entity_aabb_misses() {
        let mut g = crate::tile_grid::TileGrid::new();
        g.set_tile("wall".to_string(), 0, 0, 0, 0);
        let transform = Mat4::IDENTITY;
        let query = Collider::Box {
            center: Vec3::new(1000., 1000., 1000.),
            half: Vec3::splat(2.),
        };
        assert!(test_grid(&query, &transform, &g).is_empty());
    }

    #[test]
    fn test_grid_rotates_the_normal_back_to_world_space() {
        let mut g = crate::tile_grid::TileGrid::new();
        g.set_tile("wall".to_string(), 0, 0, 0, 0); // local tile center (8,8,8), half 8
        // Grid rotated 90 degrees around Z and moved to world (100,0,0).
        let transform =
            Mat4::from_translation(Vec3::new(100., 0., 0.)) * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        // A query that, in the grid's local space, sits to the tile's -X side
        // (local center (2,8,8), half 2) — world center is that local point
        // carried through `transform`.
        let world_center = transform.transform_point3(Vec3::new(2., 8., 8.));
        approx(world_center, Vec3::new(92., 2., 8.));
        let query = Collider::Box {
            center: world_center,
            half: Vec3::splat(2.),
        };
        let hits = test_grid(&query, &transform, &g);
        assert_eq!(hits.len(), 1);
        let hit = hits[0].3;
        // Local normal would be -X (push away from the tile, which is to the
        // +X side); rotate_z(90) turns -X into -Y in world space.
        approx(hit.normal, Vec3::new(0., -1., 0.));
        assert!((hit.depth - 4.).abs() < 1e-4, "depth {}", hit.depth);
    }

    #[test]
    fn tile_box_matches_the_tile_to_world_convention() {
        // tile.rs: "index position would be position divided by 16" — so
        // tile (2,0,0) spans world [32,48) on X.
        let b = tile_box(2, 0, 0);
        match b {
            Collider::Box { center, half } => {
                approx(center, Vec3::new(40., 8., 8.));
                approx(half, Vec3::splat(8.));
            }
            _ => panic!("tile_box must return a Box"),
        }
    }
}

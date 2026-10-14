//! Tilesets: tiles that mesh together on the dual grid instead of as cubes.
//!
//! A *dual cube* spans the centres of 8 neighbouring cells. Which of those
//! cells are solid gives an 8-bit *case* (bit `x + 2y + 4z` for the corner at
//! `(x, y, z)` in `{0,1}³`), and each case is drawn with one *piece*: a mesh
//! that fills the dual cube in local units `0..16`, where `0` is the centre of
//! the cube's lowest cell (the same space a model tile occupies). Every cell
//! face ends up covered by the four dual cubes that share it, so the pieces
//! join into one surface.
//!
//! A tileset's pieces come from, in order:
//! 1. the user's models, by case, each also covering its 90° turns about z
//!    (the turns `tile()`'s `rot` already does, `Vertex::rotp90` and kin);
//! 2. a generated fallback, the tileset's `look`:
//!    - `Block`: the octant faces of each solid corner — exactly the cube
//!      look, minus the faces two solid cells share;
//!    - `Smooth`: marching cubes on cell centres, flat shaded, with 45° slopes.
//!
//! Every non-empty cell counts as solid for the case, members or not, so a
//! tileset never opens a hole against an ordinary tile. A dual cube with no
//! member corner draws nothing (its cells draw as cubes or models as before),
//! and one shared by two tilesets is drawn by the one with more corners in it.
//!
//! Collision, rays and `istile` still treat every tile as its cube: this is a
//! look, the tiles underneath are unchanged.

use glam::{ivec3, IVec3, Vec3, Vec4};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

use crate::model::{ModelManager, Vertex};

/// One dual cube's local extent, in vertex units (a cell is 16).
const S: i16 = 16;
/// The middle of a dual cube: the planes between its cells.
const H: i16 = 8;

/// How a tileset draws a case it has no model for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Look {
    Block,
    Smooth,
}

impl Look {
    pub fn parse(s: &str) -> Look {
        match s {
            "smooth" => Look::Smooth,
            _ => Look::Block,
        }
    }
}

/// A tileset as Lua defines it (`tileset(name, tiles, opts)`), by names. An
/// empty `tiles` removes the set.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TilesetDef {
    pub name: String,
    pub tiles: Vec<String>,
    pub look: Look,
    /// (case, model name)
    pub pieces: Vec<(u8, String)>,
}

struct Tileset {
    name: String,
    tiles: Vec<String>,
    look: Look,
    /// The model drawn for each case and how many 90° turns it needs.
    pieces: Vec<Option<(String, u8)>>,
}

/// A world's tilesets, and which tile index belongs to which.
#[derive(Default)]
pub struct Tilesets {
    sets: Vec<Tileset>,
    /// tile index -> position in `sets`
    members: FxHashMap<u32, u16>,
}

impl Tilesets {
    pub fn is_empty(&self) -> bool {
        self.sets.is_empty()
    }

    /// Add, replace (same name) or remove (no tiles) a tileset.
    pub fn define(&mut self, def: TilesetDef) {
        let at = self.sets.iter().position(|s| s.name == def.name);
        if def.tiles.is_empty() {
            if let Some(i) = at {
                self.sets.remove(i);
            }
            return;
        }
        let set = Tileset {
            name: def.name,
            tiles: def.tiles.iter().map(|t| t.to_lowercase()).collect(),
            look: def.look,
            pieces: piece_table(&def.pieces),
        };
        match at {
            Some(i) => self.sets[i] = set,
            None => self.sets.push(set),
        }
    }

    /// Re-read which tile index each member name has; names are indexed as
    /// they're first used, so this runs before each meshing pass. A name in
    /// two tilesets belongs to the later one.
    pub fn resolve(&mut self, names: &FxHashMap<String, u32>) {
        self.members.clear();
        for (i, set) in self.sets.iter().enumerate() {
            for t in &set.tiles {
                if let Some(&ind) = names.get(t) {
                    self.members.insert(ind, i as u16);
                }
            }
        }
    }

    pub fn member(&self, cell: u32) -> Option<u16> {
        if cell == 0 {
            return None;
        }
        self.members.get(&cell).copied()
    }
}

/// Fill every case a user piece covers, its own case first, then the turns of
/// each piece for cases nobody gave explicitly.
fn piece_table(pieces: &[(u8, String)]) -> Vec<Option<(String, u8)>> {
    let mut table: Vec<Option<(String, u8)>> = vec![None; 256];
    for (case, name) in pieces {
        table[*case as usize] = Some((name.to_lowercase(), 0));
    }
    for (case, name) in pieces {
        let mut c = *case;
        for turns in 1..4u8 {
            c = turn_case(c);
            if table[c as usize].is_none() {
                table[c as usize] = Some((name.to_lowercase(), turns));
            }
        }
    }
    table
}

/// The corner bit for `(x, y, z)` in `{0,1}³`.
pub fn corner_bit(x: usize, y: usize, z: usize) -> u8 {
    1 << (x + 2 * y + 4 * z)
}

/// The case a piece for `case` covers once turned by `Vertex::rotp90`, which
/// maps local `(x, y)` to `(16 - y, x)`: corner `(x, y)` lands on `(1 - y, x)`.
pub fn turn_case(case: u8) -> u8 {
    let mut out = 0;
    for i in 0..8 {
        if case & (1 << i) != 0 {
            let (x, y, z) = (i & 1, (i >> 1) & 1, (i >> 2) & 1);
            out |= corner_bit(1 - y, x, z);
        }
    }
    out
}

/// Parse a case from Lua: an integer 0..255, or 8 `0`/`1` characters giving
/// corners in bit order — the bottom layer `(0,0) (1,0) (0,1) (1,1)` then the
/// top — with spaces, `_` and `/` ignored, so `"1111 0000"` is a floor slab.
pub fn parse_case(s: &str) -> Option<u8> {
    if let Ok(n) = s.trim().parse::<u32>() {
        if s.trim().len() < 8 {
            return if n < 256 { Some(n as u8) } else { None };
        }
    }
    let bits: Vec<char> = s
        .chars()
        .filter(|c| !matches!(c, ' ' | '_' | '/'))
        .collect();
    if bits.len() != 8 {
        return None;
    }
    let mut case = 0u8;
    for (i, c) in bits.iter().enumerate() {
        match c {
            '1' => case |= 1 << i,
            '0' => {}
            _ => return None,
        }
    }
    Some(case)
}

// -- generated pieces

/// A vertex of a generated piece; `corner` says whose texture it wears.
#[derive(Clone, Copy, Debug)]
pub struct GenVert {
    pub pos: [i16; 3],
    pub normal: [i8; 3],
    /// 0..1 within the tile's texture
    pub uv: [f32; 2],
    pub corner: u8,
    /// The corner whose eighth of the dual cube its triangle lies in, on the
    /// solid side. Where that's an ordinary tile, the tile's own cube is
    /// already drawn there, and drawing this too would z-fight.
    pub cell: u8,
}

#[derive(Clone, Debug, Default)]
pub struct GenPiece {
    pub verts: Vec<GenVert>,
    pub inds: Vec<u32>,
}

fn corner_pos(i: usize) -> [i16; 3] {
    [(i & 1) as i16 * S, ((i >> 1) & 1) as i16 * S, ((i >> 2) & 1) as i16 * S]
}

/// Texture coordinates on a cube face, matching `model::create_cube`'s, for
/// `p` within a 0..16 cell and the face's normal axis and sign.
fn face_uv(p: [i16; 3], axis: usize, positive: bool) -> [f32; 2] {
    let f = |v: i16| v as f32 / S as f32;
    match (axis, positive) {
        (2, _) => [f(p[0]), 1. - f(p[1])],
        (0, _) => [f(p[1]), 1. - f(p[2])],
        (1, true) => [1. - f(p[0]), 1. - f(p[2])],
        (1, false) => [f(p[0]), 1. - f(p[2])],
        _ => [0., 0.],
    }
}

/// The block look: each solid corner's octant shows a face toward each empty
/// neighbour corner, on the plane between the two cells.
fn block_piece(case: u8) -> GenPiece {
    let mut piece = GenPiece::default();
    for i in 0..8usize {
        if case & (1 << i) == 0 {
            continue;
        }
        let o = [(i & 1) as i16, ((i >> 1) & 1) as i16, ((i >> 2) & 1) as i16];
        for axis in 0..3 {
            if case & (1 << (i ^ (1 << axis))) != 0 {
                continue;
            }
            let positive = o[axis] == 0;
            // tangents with u × v = +axis; flip them for a face looking down it
            let (mut u, mut v) = ((axis + 1) % 3, (axis + 2) % 3);
            if !positive {
                std::mem::swap(&mut u, &mut v);
            }
            let lo = |a: usize| o[a] * H;
            let mut normal = [0i8; 3];
            normal[axis] = if positive { 127 } else { -127 };
            let base = piece.verts.len() as u32;
            for (du, dv) in [(0, 0), (1, 0), (1, 1), (0, 1)] {
                let mut p = [0i16; 3];
                p[axis] = H;
                p[u] = lo(u) + du * H;
                p[v] = lo(v) + dv * H;
                // the same point within the corner's own cell
                let cell = [p[0] + H - o[0] * S, p[1] + H - o[1] * S, p[2] + H - o[2] * S];
                piece.verts.push(GenVert {
                    pos: p,
                    normal,
                    uv: face_uv(cell, axis, positive),
                    corner: i as u8,
                    cell: i as u8,
                });
            }
            piece
                .inds
                .extend_from_slice(&[base, base + 1, base + 2, base + 2, base + 3, base]);
        }
    }
    piece
}

/// The cube's faces as corner indices, counter-clockwise seen from outside.
const FACES: [[usize; 4]; 6] = [
    [0, 4, 6, 2], // -x
    [1, 3, 7, 5], // +x
    [0, 1, 5, 4], // -y
    [2, 6, 7, 3], // +y
    [0, 2, 3, 1], // -z
    [4, 5, 7, 6], // +z
];

/// The smooth look: marching cubes with each crossing at its edge's middle.
/// Rather than the usual 256-row table, each face of the cube is marched as a
/// square — a segment from where its boundary enters solid to where it next
/// leaves, so diagonal solids stay apart and both cubes sharing a face agree —
/// and the segments chain into loops (every edge crossing lies on exactly two
/// faces), each fanned into triangles. Wound so the front faces the air.
fn smooth_piece(case: u8) -> GenPiece {
    let mut piece = GenPiece::default();
    if case == 0 || case == 255 {
        return piece;
    }
    let solid = |i: usize| case & (1 << i) != 0;
    let key = |a: usize, b: usize| if a < b { (a, b) } else { (b, a) };
    // edge -> the edge its segment leads to
    let mut next: FxHashMap<(usize, usize), (usize, usize)> = FxHashMap::default();
    for face in FACES {
        let crossings: Vec<(usize, bool)> = (0..4)
            .filter(|&k| solid(face[k]) != solid(face[(k + 1) % 4]))
            .map(|k| (k, solid(face[(k + 1) % 4])))
            .collect();
        for (n, &(k, entering)) in crossings.iter().enumerate() {
            if entering {
                let (k2, _) = crossings[(n + 1) % crossings.len()];
                next.insert(
                    key(face[k], face[(k + 1) % 4]),
                    key(face[k2], face[(k2 + 1) % 4]),
                );
            }
        }
    }
    let mid = |(a, b): (usize, usize)| {
        let (pa, pb) = (corner_pos(a), corner_pos(b));
        [(pa[0] + pb[0]) / 2, (pa[1] + pb[1]) / 2, (pa[2] + pb[2]) / 2]
    };
    let mut starts: Vec<(usize, usize)> = next.keys().copied().collect();
    starts.sort();
    let mut seen: Vec<(usize, usize)> = vec![];
    for start in starts {
        if seen.contains(&start) {
            continue;
        }
        let mut ring = vec![];
        let mut e = start;
        loop {
            seen.push(e);
            ring.push(mid(e));
            e = next[&e];
            if e == start {
                break;
            }
        }
        for t in 1..ring.len() - 1 {
            for tri in split_eighths([ring[0], ring[t], ring[t + 1]]) {
                push_flat(&mut piece, tri, case);
            }
        }
    }
    piece
}

/// Cut a triangle along the dual cube's three middle planes, so each piece
/// lies within one corner's eighth of it. Cut points fall on the 1/16 grid
/// (the corners are 0, 8 or 16 on each axis, so a cut halves an edge), and a
/// neighbouring cube cuts a shared edge at the same point, so it stays closed.
fn split_eighths(tri: [[i16; 3]; 3]) -> Vec<[[i16; 3]; 3]> {
    let mut polys: Vec<Vec<[i16; 3]>> = vec![tri.to_vec()];
    for axis in 0..3 {
        let mut next = vec![];
        for poly in polys {
            if poly.iter().all(|p| p[axis] == H) {
                // in the plane: nothing to cut
                next.push(poly);
                continue;
            }
            for low in [true, false] {
                let side = clip(&poly, axis, low);
                if side.len() >= 3 {
                    next.push(side);
                }
            }
        }
        polys = next;
    }
    let mut tris = vec![];
    for poly in polys {
        for t in 1..poly.len() - 1 {
            let tri = [poly[0], poly[t], poly[t + 1]];
            let v = |p: [i16; 3]| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
            if (v(tri[1]) - v(tri[0])).cross(v(tri[2]) - v(tri[0])).length_squared() > 0.01 {
                tris.push(tri);
            }
        }
    }
    tris
}

/// The part of a polygon on one side of the plane `axis = 8`, in order.
fn clip(poly: &[[i16; 3]], axis: usize, low: bool) -> Vec<[i16; 3]> {
    let inside = |p: &[i16; 3]| if low { p[axis] <= H } else { p[axis] >= H };
    let mut out: Vec<[i16; 3]> = vec![];
    let mut push = |p: [i16; 3], out: &mut Vec<[i16; 3]>| {
        if out.last() != Some(&p) && !(out.len() > 0 && out[0] == p) {
            out.push(p);
        }
    };
    for k in 0..poly.len() {
        let (a, b) = (poly[k], poly[(k + 1) % poly.len()]);
        if inside(&a) {
            push(a, &mut out);
        }
        if (a[axis] < H && b[axis] > H) || (a[axis] > H && b[axis] < H) {
            let t = (H - a[axis]) as f32 / (b[axis] - a[axis]) as f32;
            let at = |i: usize| (a[i] as f32 + (b[i] - a[i]) as f32 * t).round() as i16;
            push([at(0), at(1), at(2)], &mut out);
        }
    }
    out
}

/// One flat-shaded triangle, textured from the solid corner nearest to it,
/// projected along its normal's strongest axis. A face looking up wears the
/// nearest corner with nothing solid over it in the cube (the grass, not the
/// dirt under it).
fn push_flat(piece: &mut GenPiece, tri: [[i16; 3]; 3], case: u8) {
    let v = |p: [i16; 3]| Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32);
    let (a, b, c) = (v(tri[0]), v(tri[1]), v(tri[2]));
    let n = (b - a).cross(c - a).normalize_or_zero();
    let centre = (a + b + c) / 3.;
    let solid = |i: usize| case & (1 << i) != 0;
    let topmost = |i: usize| i >= 4 || !solid(i + 4);
    let nearest = |only_top: bool| {
        (0..8usize)
            .filter(|&i| solid(i) && (!only_top || topmost(i)))
            .min_by(|&i, &j| {
                let d = |k: usize| v(corner_pos(k)).distance_squared(centre);
                d(i).total_cmp(&d(j))
            })
    };
    let corner = if n.z > 0.3 { nearest(true) } else { None }
        .or_else(|| nearest(false))
        .unwrap_or(0) as u8;
    let axis = if n.x.abs() >= n.y.abs() && n.x.abs() >= n.z.abs() {
        0
    } else if n.y.abs() >= n.z.abs() {
        1
    } else {
        2
    };
    let normal = [
        (n.x * 127.).round() as i8,
        (n.y * 127.).round() as i8,
        (n.z * 127.).round() as i8,
    ];
    // the eighth it lies in, from just behind it (a triangle in a middle
    // plane belongs to the solid side)
    let behind = centre - n * 0.5;
    let cell = (behind.x >= H as f32) as u8
        | ((behind.y >= H as f32) as u8) << 1
        | ((behind.z >= H as f32) as u8) << 2;
    let base = piece.verts.len() as u32;
    for p in tri {
        piece.verts.push(GenVert {
            pos: p,
            normal,
            uv: face_uv(p, axis, n[axis] > 0.),
            corner,
            cell,
        });
    }
    piece.inds.extend_from_slice(&[base, base + 1, base + 2]);
}

pub fn generated(look: Look, case: u8) -> &'static GenPiece {
    static BLOCK: OnceLock<Vec<GenPiece>> = OnceLock::new();
    static SMOOTH: OnceLock<Vec<GenPiece>> = OnceLock::new();
    let table = match look {
        Look::Block => BLOCK.get_or_init(|| (0..=255u8).map(block_piece).collect()),
        Look::Smooth => SMOOTH.get_or_init(|| (0..=255u8).map(smooth_piece).collect()),
    };
    &table[case as usize]
}

// -- meshing a chunk

/// A chunk's cells and one more on each high side, read from its neighbours:
/// the corners of every dual cube whose lowest cell is in the chunk.
pub struct Apron {
    size: i32,
    cells: Vec<u32>,
}

impl Apron {
    /// `cell(x, y, z)` gives the tile index at local `(x, y, z)` for
    /// `0..=size` on each axis.
    pub fn new(size: i32, cell: impl Fn(i32, i32, i32) -> u32) -> Apron {
        let n = size + 1;
        let mut cells = vec![0; (n * n * n) as usize];
        for x in 0..n {
            for y in 0..n {
                for z in 0..n {
                    cells[((x * n + y) * n + z) as usize] = cell(x, y, z);
                }
            }
        }
        Apron { size, cells }
    }

    pub fn get(&self, x: i32, y: i32, z: i32) -> u32 {
        let n = self.size + 1;
        self.cells[((x * n + y) * n + z) as usize]
    }
}

/// Append the tilesets' pieces for every dual cube whose lowest cell is in
/// the chunk. `offset` is where cell `(0,0,0)`'s centre sits in the chunk
/// mesh. Returns nothing; `verts`/`inds` grow.
pub fn mesh(
    sets: &Tilesets,
    apron: &Apron,
    tex_map: &FxHashMap<u32, Vec4>,
    model_manager: &ModelManager,
    offset: IVec3,
    verts: &mut Vec<Vertex>,
    inds: &mut Vec<u32>,
) {
    if sets.is_empty() {
        return;
    }
    let size = apron.size;
    let mut counts = vec![0u8; sets.sets.len()];
    for x in 0..size {
        for y in 0..size {
            for z in 0..size {
                let mut case = 0u8;
                let mut cells = [0u32; 8];
                counts.iter_mut().for_each(|c| *c = 0);
                let mut any = false;
                for i in 0..8usize {
                    let cell =
                        apron.get(x + (i & 1) as i32, y + ((i >> 1) & 1) as i32, z + (i >> 2) as i32);
                    cells[i] = cell;
                    if cell != 0 {
                        case |= 1 << i;
                    }
                    if let Some(s) = sets.member(cell) {
                        counts[s as usize] += 1;
                        any = true;
                    }
                }
                if !any || case == 255 {
                    continue;
                }
                // the tileset with the most corners here; the first on a tie
                let mut owner = 0;
                for (s, &c) in counts.iter().enumerate() {
                    if c > counts[owner] {
                        owner = s;
                    }
                }
                let set = &sets.sets[owner];
                let at = offset + ivec3(x, y, z) * S as i32;

                if let Some((name, turns)) = &set.pieces[case as usize] {
                    if let Some(model) = model_manager.get_model_or_not(name) {
                        if let Some(data) = model.data.as_ref() {
                            let base = verts.len() as u32;
                            for v in &data.0 {
                                let mut v = *v;
                                match turns {
                                    1 => v.rotp90(),
                                    2 => v.rotp180(),
                                    3 => v.rotp270(),
                                    _ => {}
                                }
                                v.trans(at);
                                verts.push(v);
                            }
                            inds.extend(data.1.iter().map(|i| i + base));
                            continue;
                        }
                    }
                }

                let piece = generated(set.look, case);
                // the block look only draws member corners; ordinary tiles
                // draw their own cubes
                let mut kept = vec![u32::MAX; piece.verts.len()];
                for (k, g) in piece.verts.iter().enumerate() {
                    let under = cells[g.cell as usize];
                    if under != 0 && sets.member(under).is_none() {
                        // an ordinary tile's cube already covers this
                        continue;
                    }
                    let mut cell = cells[g.corner as usize];
                    if sets.member(cell).is_none() {
                        if set.look == Look::Block {
                            continue;
                        }
                        // a smooth slope by an ordinary tile wears the set's own
                        cell = (0..8)
                            .map(|i| cells[i])
                            .find(|&c| sets.member(c) == Some(owner as u16))
                            .unwrap_or(cell);
                    }
                    let uv = tex_map.get(&cell).copied().unwrap_or(Vec4::new(1., 1., 0., 0.));
                    kept[k] = verts.len() as u32;
                    verts.push(Vertex::new(
                        [g.pos[0] + at.x as i16, g.pos[1] + at.y as i16, g.pos[2] + at.z as i16],
                        g.normal,
                        [g.uv[0] * uv.z + uv.x, g.uv[1] * uv.w + uv.y],
                    ));
                }
                for tri in piece.inds.chunks_exact(3) {
                    let t = [kept[tri[0] as usize], kept[tri[1] as usize], kept[tri[2] as usize]];
                    if t.iter().all(|&i| i != u32::MAX) {
                        inds.extend_from_slice(&t);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Directed edges of a piece's triangles, by position.
    fn edges(p: &GenPiece) -> Vec<([i16; 3], [i16; 3])> {
        p.inds
            .chunks_exact(3)
            .flat_map(|t| {
                let v = |k: u32| p.verts[k as usize].pos;
                [(v(t[0]), v(t[1])), (v(t[1]), v(t[2])), (v(t[2]), v(t[0]))]
            })
            .collect()
    }

    /// Mesh a few solid cells (a 0/1 grid of `n`³ cells) with one look:
    /// every dual cube from -1, so the outside is covered too. Positions are
    /// in cell-centre space, 16 a cell.
    fn mesh_cells(look: Look, solid: &dyn Fn(i32, i32, i32) -> bool, n: i32) -> GenPiece {
        let mut out = GenPiece::default();
        for x in -1..n {
            for y in -1..n {
                for z in -1..n {
                    let mut case = 0u8;
                    for i in 0..8usize {
                        let (cx, cy, cz) =
                            (x + (i & 1) as i32, y + ((i >> 1) & 1) as i32, z + (i >> 2) as i32);
                        if solid(cx, cy, cz) {
                            case |= 1 << i;
                        }
                    }
                    let p = generated(look, case);
                    let base = out.verts.len() as u32;
                    for v in &p.verts {
                        let mut v = *v;
                        v.pos = [
                            v.pos[0] + (x * 16) as i16,
                            v.pos[1] + (y * 16) as i16,
                            v.pos[2] + (z * 16) as i16,
                        ];
                        out.verts.push(v);
                    }
                    out.inds.extend(p.inds.iter().map(|i| i + base));
                }
            }
        }
        out
    }

    /// Closed and consistently wound: each directed edge is matched by as
    /// many going back. (Blocks touching only along an edge share it four
    /// ways, as cube tiles do.)
    fn assert_watertight(p: &GenPiece) {
        let e = edges(p);
        assert!(!e.is_empty());
        for (a, b) in &e {
            let there = e.iter().filter(|x| **x == (*a, *b)).count();
            let back = e.iter().filter(|x| **x == (*b, *a)).count();
            assert_eq!(there, back, "open edge {a:?}->{b:?}");
        }
    }

    /// The smooth look never shares an edge between more than two triangles.
    fn assert_manifold(p: &GenPiece) {
        let e = edges(p);
        for d in &e {
            assert_eq!(e.iter().filter(|x| *x == d).count(), 1, "edge {d:?} used twice");
        }
    }

    fn inside(n: i32, x: i32, y: i32, z: i32) -> bool {
        (0..n).contains(&x) && (0..n).contains(&y) && (0..n).contains(&z)
    }

    /// Six times the signed volume of a closed mesh; positive when its faces
    /// point outward.
    fn volume6(p: &GenPiece) -> i64 {
        p.inds
            .chunks_exact(3)
            .map(|t| {
                let v = |k: u32| {
                    let q = p.verts[k as usize].pos;
                    [q[0] as i64, q[1] as i64, q[2] as i64]
                };
                let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
                    + a[2] * (b[0] * c[1] - b[1] * c[0])
            })
            .sum()
    }

    #[test]
    fn one_block_is_a_closed_cube_of_one_cell() {
        let p = mesh_cells(Look::Block, &|x, y, z| (x, y, z) == (0, 0, 0), 1);
        assert_watertight(&p);
        assert_eq!(p.inds.len() / 6, 24, "each face in four quarters");
        assert_eq!(volume6(&p), 6 * 16 * 16 * 16);
        // spans the cell around its centre, like a cube tile
        let xs: Vec<i16> = p.verts.iter().map(|v| v.pos[0]).collect();
        assert_eq!((*xs.iter().min().unwrap(), *xs.iter().max().unwrap()), (-8, 8));
    }

    #[test]
    fn blocks_hide_the_faces_they_share() {
        let p = mesh_cells(Look::Block, &|x, y, z| inside(2, x, y, z) && y == 0 && z == 0, 2);
        assert_watertight(&p);
        // 10 faces of a 2x1x1 bar
        assert_eq!(p.inds.len() / 6, 40);
        assert_eq!(volume6(&p), 6 * 2 * 16 * 16 * 16);
    }

    #[test]
    fn block_uvs_match_the_cube_tile() {
        // the top of one cell: four quarters whose uvs tile 0..1 as create_cube's top does
        let p = mesh_cells(Look::Block, &|x, y, z| (x, y, z) == (0, 0, 0), 1);
        for v in p.verts.iter().filter(|v| v.normal == [0, 0, 127]) {
            let (x, y) = ((v.pos[0] + 8) as f32 / 16., (v.pos[1] + 8) as f32 / 16.);
            assert_eq!(v.uv, [x, 1. - y]);
        }
    }

    #[test]
    fn smooth_shapes_are_closed_and_face_out() {
        let shapes: [(&str, &dyn Fn(i32, i32, i32) -> bool, i32); 4] = [
            ("one cell", &|x, y, z| (x, y, z) == (0, 0, 0), 1),
            ("a slab", &|x, y, z| inside(3, x, y, z) && z == 0, 3),
            ("a diagonal pair", &|x, y, z| (x, y, z) == (0, 0, 0) || (x, y, z) == (1, 1, 0), 2),
            ("a corner-touching pair", &|x, y, z| (x, y, z) == (0, 0, 0) || (x, y, z) == (1, 1, 1), 2),
        ];
        for (name, solid, n) in shapes {
            let p = mesh_cells(Look::Smooth, solid, n);
            assert_watertight(&p);
            assert_manifold(&p);
            assert!(volume6(&p) > 0, "{name} faces inward");
        }
    }

    #[test]
    fn every_case_is_closed_with_its_neighbours() {
        // any 2x2x2 arrangement of cells, meshed with all 27 dual cubes around it
        for case in 1..=255u8 {
            let solid = move |x: i32, y: i32, z: i32| {
                (0..2).contains(&x)
                    && (0..2).contains(&y)
                    && (0..2).contains(&z)
                    && case & corner_bit(x as usize, y as usize, z as usize) != 0
            };
            for look in [Look::Block, Look::Smooth] {
                let p = mesh_cells(look, &solid, 2);
                assert_watertight(&p);
                if look == Look::Smooth {
                    assert_manifold(&p);
                }
                assert!(volume6(&p) > 0, "case {case} {look:?} faces inward");
            }
        }
    }

    #[test]
    fn smooth_triangles_each_lie_in_the_eighth_they_name() {
        for case in 1..255u8 {
            let p = generated(Look::Smooth, case);
            for t in p.inds.chunks_exact(3) {
                let cell = p.verts[t[0] as usize].cell as usize;
                for &k in t {
                    let v = p.verts[k as usize];
                    assert_eq!(v.cell as usize, cell);
                    for a in 0..3 {
                        let lo = ((cell >> a) & 1) as i16 * H;
                        assert!((lo..=lo + H).contains(&v.pos[a]), "case {case}: {:?} outside eighth {cell}", v.pos);
                    }
                }
            }
        }
    }

    #[test]
    fn a_flat_top_splits_into_its_four_cells() {
        // the floor slab: its top at z=8, a quarter over each bottom cell, so
        // the quarter over an ordinary tile can be left to that tile's cube
        let p = generated(Look::Smooth, 0b0000_1111);
        for cell in 0..4u8 {
            let area2: i32 = p
                .inds
                .chunks_exact(3)
                .filter(|t| p.verts[t[0] as usize].cell == cell)
                .map(|t| {
                    let v = |k: u32| p.verts[k as usize].pos;
                    let (a, b, c) = (v(t[0]), v(t[1]), v(t[2]));
                    ((b[0] - a[0]) as i32 * (c[1] - a[1]) as i32
                        - (b[1] - a[1]) as i32 * (c[0] - a[0]) as i32)
                        .abs()
                })
                .sum();
            assert_eq!(area2, 2 * 8 * 8, "cell {cell}");
        }
    }

    #[test]
    fn turning_a_case_four_times_is_itself() {
        for case in 0..=255u8 {
            assert_eq!(turn_case(turn_case(turn_case(turn_case(case)))), case);
        }
        // the floor slab is the same turned; a single corner walks the square
        assert_eq!(turn_case(0b0000_1111), 0b0000_1111);
        assert_eq!(turn_case(corner_bit(0, 0, 0)), corner_bit(1, 0, 0));
        assert_eq!(turn_case(corner_bit(1, 0, 0)), corner_bit(1, 1, 0));
    }

    #[test]
    fn a_turned_piece_covers_the_turned_case() {
        // turning the generated piece for a case by rotp90 gives the piece's
        // shape for turn_case (the block look: its vertices depend on the case alone)
        for case in [1u8, 3, 7, 0b0001_0011, 0b1000_0001] {
            let piece = generated(Look::Block, case);
            let mut turned: Vec<[i16; 3]> = piece
                .verts
                .iter()
                .map(|g| {
                    let mut v = Vertex::new(g.pos, g.normal, g.uv);
                    v.rotp90();
                    let p = v.pos();
                    [p[0], p[1], p[2]]
                })
                .collect();
            let mut want: Vec<[i16; 3]> =
                generated(Look::Block, turn_case(case)).verts.iter().map(|g| g.pos).collect();
            turned.sort();
            turned.dedup();
            want.sort();
            want.dedup();
            assert_eq!(turned, want, "case {case}");
        }
    }

    #[test]
    fn user_pieces_fill_their_turns_but_not_over_explicit_ones() {
        let one = corner_bit(0, 0, 0);
        let t = piece_table(&[(one, "Nub".into()), (corner_bit(1, 1, 0), "other".into())]);
        assert_eq!(t[one as usize], Some(("nub".into(), 0)));
        assert_eq!(t[corner_bit(1, 0, 0) as usize], Some(("nub".into(), 1)));
        assert_eq!(t[corner_bit(1, 1, 0) as usize], Some(("other".into(), 0)));
        assert_eq!(t[corner_bit(0, 1, 0) as usize], Some(("nub".into(), 3)));
        assert_eq!(t[corner_bit(0, 0, 1) as usize], None);
    }

    #[test]
    fn cases_parse_from_bits_or_numbers() {
        assert_eq!(parse_case("1111 0000"), Some(0b0000_1111));
        assert_eq!(parse_case("1000/0000"), Some(1));
        assert_eq!(parse_case("0000_0001"), Some(0b1000_0000));
        assert_eq!(parse_case("15"), Some(15));
        assert_eq!(parse_case("256"), None);
        assert_eq!(parse_case("10x00000"), None);
    }

    #[test]
    fn tilesets_resolve_names_and_later_sets_win() {
        let mut sets = Tilesets::default();
        let def = |name: &str, tiles: &[&str]| TilesetDef {
            name: name.into(),
            tiles: tiles.iter().map(|t| t.to_string()).collect(),
            look: Look::Block,
            pieces: vec![],
        };
        sets.define(def("a", &["Dirt", "stone"]));
        sets.define(def("b", &["stone"]));
        let mut names = FxHashMap::default();
        names.insert("dirt".to_string(), 5);
        names.insert("stone".to_string(), 6);
        sets.resolve(&names);
        assert_eq!(sets.member(5), Some(0));
        assert_eq!(sets.member(6), Some(1));
        assert_eq!(sets.member(0), None);
        sets.define(def("a", &[]));
        sets.resolve(&names);
        assert_eq!(sets.member(5), None);
        assert_eq!(sets.member(6), Some(0));
    }
}

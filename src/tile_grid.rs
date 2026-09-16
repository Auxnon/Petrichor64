//! An entity-owned tile grid — the moving/rotating counterpart to the static
//! world grid (`tile.rs`'s `Chunk`/`Layer`). See `guide/entity.md` and the
//! session plan this was built from.
//!
//! Deliberately **not** a reuse of `Chunk` verbatim, despite that being the
//! original plan: `Chunk::cells` resolves tile names to a `u32` index via a
//! `WorldInstance` (name↔index registry that only exists on the world's
//! dedicated thread), and `LuaEnt` methods — where this data lives, see
//! below — have no path to any main-thread/thread-owned state at all (the
//! same "return-value wall" that made `ent:hit()` a top-level function
//! instead of a colon method). So a grid cell stores its asset name directly
//! (`GridCell::asset`), resolved to an atlas/model lazily, only when
//! `EntManager` rebuilds the render mesh (which does have `ModelManager`/
//! `TexManager` in hand) — mirroring how the world's own per-bundle `Mapper`
//! resolution already happens at `process_sync` time, not at `set_tile` time.
//!
//! A `String` cell is far heavier than the world's `(u32,u8)`, so chunks here
//! are **sparse** (`FxHashMap<u32, GridCell>` of only the non-air cells)
//! rather than a dense `[_; 32768]` array — a moving platform is expected to
//! be mostly empty relative to a full 32³ chunk volume, unlike world terrain.

use rustc_hash::FxHashMap;

// Plain `[i32;3]`, not `glam::IVec3`: `LuaEnt` (where `TileGrid` lives)
// derives Serialize/Deserialize for the wasm worker boundary, and this
// project's `glam` dependency doesn't enable its `serde` feature — every
// other position field on `LuaEnt` already avoids glam types for the same
// reason (`size`/`offset`/`hit_size` are all `[f64;3]`, not `Vec3`).

pub const GRID_CHUNK_SIZE: i32 = 32;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GridCell {
    pub asset: String,
    pub meta: u8,
}

/// One chunk of an entity's grid, in the entity's own local (tile-index)
/// space — no relation to world-space coordinates at all.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct GridChunk {
    pub dirty: bool,
    pub cells: FxHashMap<u32, GridCell>,
    /// This chunk's local-space origin (tile-index units, `GRID_CHUNK_SIZE`-aligned).
    pub pos: [i32; 3],
    pub key: String,
}

fn local_index(ix: i32, iy: i32, iz: i32) -> u32 {
    ((ix.rem_euclid(GRID_CHUNK_SIZE) * GRID_CHUNK_SIZE + iy.rem_euclid(GRID_CHUNK_SIZE))
        * GRID_CHUNK_SIZE
        + iz.rem_euclid(GRID_CHUNK_SIZE)) as u32
}

/// Inverse of `local_index` — recovers a cell's local `(ix,iy,iz)` (each
/// `0..GRID_CHUNK_SIZE`) from its packed key. Used by the render mesher and
/// the collision cascade (`collide::test_grid`), both of which walk
/// `GridChunk::cells` and need the position back, not just the packed index.
pub fn unpack_local_index(idx: u32) -> (i32, i32, i32) {
    let idx = idx as i32;
    let iz = idx % GRID_CHUNK_SIZE;
    let iy = (idx / GRID_CHUNK_SIZE) % GRID_CHUNK_SIZE;
    let ix = idx / (GRID_CHUNK_SIZE * GRID_CHUNK_SIZE);
    (ix, iy, iz)
}

/// The chunk-grid key for a local tile index — same `div_euclid`/`"{rx}:{ry}:{rz}"`
/// convention `tile::chunk_key` uses for the world grid, just independently
/// scoped (an entity grid's chunk 0 has nothing to do with the world's).
pub fn grid_chunk_key(ix: i32, iy: i32, iz: i32) -> String {
    format!(
        "{}:{}:{}",
        ix.div_euclid(GRID_CHUNK_SIZE),
        iy.div_euclid(GRID_CHUNK_SIZE),
        iz.div_euclid(GRID_CHUNK_SIZE)
    )
}

/// An entity's own tile grid — see the module doc. Lives directly on
/// `LuaEnt` (plain, `Serialize`/`Deserialize`-safe data — no thread/GPU
/// handles anywhere in it), so its methods (`set_tile`/`get_tile`/etc.) never
/// need to reach across to main-thread-owned state.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct TileGrid {
    pub chunks: FxHashMap<String, GridChunk>,
    /// Cached overall bounds (tile-index units, inclusive-min/exclusive-max),
    /// for the collision cascade's first (cheapest) reject. Grows on
    /// `set_tile`; deliberately never shrinks on removal (a conservative
    /// over-estimate is cheap; exact shrink would mean rescanning every
    /// chunk on every removal for no real benefit at moving-platform scale).
    pub bounds_min: [i32; 3],
    pub bounds_max: [i32; 3],
    has_tiles: bool,
    /// Set on any mutation, cleared by `EntManager` once it rebuilds that
    /// entity's cached render mesh from this grid — same shape as the
    /// existing `asset`/`tex` dirty-flag drain in `ent_manager.rs`.
    pub dirty: bool,
}

impl TileGrid {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_tile(&mut self, asset: String, meta: u8, ix: i32, iy: i32, iz: i32) {
        let key = grid_chunk_key(ix, iy, iz);
        let idx = local_index(ix, iy, iz);
        if asset.is_empty() {
            if let Some(chunk) = self.chunks.get_mut(&key) {
                if chunk.cells.remove(&idx).is_some() {
                    chunk.dirty = true;
                    self.dirty = true;
                }
            }
            return;
        }
        let rx = ix.div_euclid(GRID_CHUNK_SIZE) * GRID_CHUNK_SIZE;
        let ry = iy.div_euclid(GRID_CHUNK_SIZE) * GRID_CHUNK_SIZE;
        let rz = iz.div_euclid(GRID_CHUNK_SIZE) * GRID_CHUNK_SIZE;
        let chunk = self.chunks.entry(key.clone()).or_insert_with(|| GridChunk {
            dirty: false,
            cells: FxHashMap::default(),
            pos: [rx, ry, rz],
            key,
        });
        chunk.cells.insert(idx, GridCell { asset, meta });
        chunk.dirty = true;
        self.dirty = true;

        let p = [ix, iy, iz];
        if self.has_tiles {
            for i in 0..3 {
                self.bounds_min[i] = self.bounds_min[i].min(p[i]);
                self.bounds_max[i] = self.bounds_max[i].max(p[i] + 1);
            }
        } else {
            self.bounds_min = p;
            self.bounds_max = [ix + 1, iy + 1, iz + 1];
            self.has_tiles = true;
        }
    }

    pub fn get_tile(&self, ix: i32, iy: i32, iz: i32) -> Option<(String, u8)> {
        self.chunks
            .get(&grid_chunk_key(ix, iy, iz))
            .and_then(|c| c.cells.get(&local_index(ix, iy, iz)))
            .map(|c| (c.asset.clone(), c.meta))
    }

    pub fn is_tile(&self, ix: i32, iy: i32, iz: i32) -> bool {
        self.chunks
            .get(&grid_chunk_key(ix, iy, iz))
            .is_some_and(|c| c.cells.contains_key(&local_index(ix, iy, iz)))
    }

    /// Mirrors `dtile`'s "drop a whole chunk" behavior.
    pub fn drop_chunk(&mut self, ix: i32, iy: i32, iz: i32) {
        if self.chunks.remove(&grid_chunk_key(ix, iy, iz)).is_some() {
            self.dirty = true;
        }
    }

    /// Mirrors `dtile()`'s no-args "clear everything" behavior.
    pub fn clear(&mut self) {
        if !self.chunks.is_empty() {
            self.chunks.clear();
            self.dirty = true;
        }
        self.has_tiles = false;
    }

    /// Mirrors `ftile`. Unlike the world's `Layer::first_tile` (which
    /// chunk-hops to stay cheap at planetary scale), this just steps
    /// directly — a grid is expected to be small (a platform, not a world),
    /// and `limit` is already capped by the caller (100, matching `ftile`),
    /// so a plain per-step lookup is simpler and plenty cheap here.
    pub fn first_tile(
        &self,
        target: Option<&str>,
        mut ix: i32,
        mut iy: i32,
        mut iz: i32,
        dx: i32,
        dy: i32,
        dz: i32,
        limit: u32,
    ) -> Option<[i32; 3]> {
        if limit == 0 || (dx == 0 && dy == 0 && dz == 0) {
            return None;
        }
        for _ in 0..limit {
            ix += dx;
            iy += dy;
            iz += dz;
            match (target, self.get_tile(ix, iy, iz)) {
                (None, None) => return Some([ix, iy, iz]),
                (Some(t), Some((asset, _))) if asset == t => return Some([ix, iy, iz]),
                _ => {}
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_is_tile_round_trip() {
        let mut g = TileGrid::new();
        assert!(!g.is_tile(1, 2, 3));
        g.set_tile("cube".to_string(), 2, 1, 2, 3);
        assert!(g.is_tile(1, 2, 3));
        assert_eq!(g.get_tile(1, 2, 3), Some(("cube".to_string(), 2)));
        assert!(!g.is_tile(1, 2, 4), "neighbor must stay empty");
    }

    #[test]
    fn empty_asset_removes_a_tile() {
        let mut g = TileGrid::new();
        g.set_tile("cube".to_string(), 0, 0, 0, 0);
        assert!(g.is_tile(0, 0, 0));
        g.set_tile(String::new(), 0, 0, 0, 0);
        assert!(!g.is_tile(0, 0, 0));
    }

    #[test]
    fn tiles_across_a_chunk_boundary_are_independent() {
        let mut g = TileGrid::new();
        // GRID_CHUNK_SIZE apart lands in a different chunk.
        g.set_tile("a".to_string(), 0, 0, 0, 0);
        g.set_tile("b".to_string(), 0, 0, GRID_CHUNK_SIZE, 0);
        assert_eq!(g.get_tile(0, 0, 0).unwrap().0, "a");
        assert_eq!(g.get_tile(0, GRID_CHUNK_SIZE, 0).unwrap().0, "b");
        assert_eq!(g.chunks.len(), 2);
    }

    #[test]
    fn bounds_grow_to_cover_every_set_tile() {
        let mut g = TileGrid::new();
        g.set_tile("a".to_string(), 0, -2, 0, 0);
        g.set_tile("b".to_string(), 0, 5, 3, -1);
        assert_eq!(g.bounds_min, [-2, 0, -1]);
        assert_eq!(g.bounds_max, [6, 4, 1]);
    }

    #[test]
    fn first_tile_finds_a_named_match_and_stops_at_limit() {
        let mut g = TileGrid::new();
        g.set_tile("wall".to_string(), 0, 3, 0, 0);
        let hit = g.first_tile(Some("wall"), 0, 0, 0, 1, 0, 0, 10);
        assert_eq!(hit, Some([3, 0, 0]));
        let miss = g.first_tile(Some("wall"), 0, 0, 0, 1, 0, 0, 2);
        assert_eq!(miss, None, "must respect the search limit");
    }

    #[test]
    fn first_tile_finds_first_empty_cell_when_target_is_none() {
        let mut g = TileGrid::new();
        g.set_tile("wall".to_string(), 0, 1, 0, 0);
        g.set_tile("wall".to_string(), 0, 2, 0, 0);
        let hit = g.first_tile(None, 0, 0, 0, 1, 0, 0, 10);
        assert_eq!(hit, Some([3, 0, 0]), "first empty cell past the two walls");
    }

    #[test]
    fn unpack_local_index_is_the_inverse_of_local_index() {
        for ix in [0, 1, 17, GRID_CHUNK_SIZE - 1] {
            for iy in [0, 5, GRID_CHUNK_SIZE - 1] {
                for iz in [0, 31, GRID_CHUNK_SIZE - 1] {
                    let idx = local_index(ix, iy, iz);
                    assert_eq!(unpack_local_index(idx), (ix, iy, iz));
                }
            }
        }
    }

    #[test]
    fn drop_chunk_and_clear() {
        let mut g = TileGrid::new();
        g.set_tile("a".to_string(), 0, 0, 0, 0);
        g.set_tile("b".to_string(), 0, 0, GRID_CHUNK_SIZE, 0);
        g.drop_chunk(0, 0, 0);
        assert!(!g.is_tile(0, 0, 0));
        assert!(g.is_tile(0, GRID_CHUNK_SIZE, 0));
        g.clear();
        assert!(!g.is_tile(0, GRID_CHUNK_SIZE, 0));
        assert!(g.chunks.is_empty());
    }
}

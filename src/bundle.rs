use std::{cell::RefCell, rc::Rc};

use image::RgbaImage;
use itertools::Itertools;
use rustc_hash::FxHashMap;
use silt_lua::userdata::WeakWrapper;

// #[cfg(feature = "headed")]
// use crate::root::Core;
use crate::{
    error::P64Error,
    gui::PreGuiMorsel,
    lua_define::{LuaCore, LuaHandle},
    pool::SharedPool,
    types::ControlState,
};

/**
 * Represent a bundle of scripts and assets occupying a single lua instance or game.
 */
pub struct Bundle {
    pub id: u8,
    pub name: String,
    directory: Option<String>,
    pub lua: LuaCore,
    pub children: Vec<u8>,
    pub rasters: FxHashMap<usize, Rc<RefCell<RgbaImage>>>,
    /** acts as a counter for for a frame skipped due to performance or intentionally */
    pub skips: u16,
    pub skipped_control_state: Option<ControlState>,
    /** Run the loop once every N frames, to bring a bundle's tick rate down. 1 is every frame. */
    pub frame_split: u16,
    pub lua_ctx_handle: Option<LuaHandle>,
    pub pool: Option<SharedPool>,
    /// An editing surface drawn *over* the app rather than part of it.
    ///
    /// Marked explicitly instead of inferred from "isn't bundle 0", because bundles
    /// will legitimately have non-overlay children one day (composition), and those
    /// must not silently start stealing input or a gui layer.
    ///
    /// Only the engine ever sets this — nothing in Lua can create a bundle, let alone
    /// an overlay. See PLAN.md's overlay trust boundary.
    pub overlay: bool,
}

pub type BundleResources = PreGuiMorsel;

impl Bundle {
    pub fn new(id: u8, name: String, lua: LuaCore, directory: Option<String>) -> Self {
        Self {
            id,
            name,
            directory,
            lua,
            children: Vec::new(),
            rasters: FxHashMap::default(),
            skips: 0,
            skipped_control_state: None,
            frame_split: 1,
            lua_ctx_handle: None,
            pool: None,
            overlay: false,
        }
    }

    pub fn get_directory(&self) -> Option<&str> {
        self.directory.as_deref()
    }

    pub fn call_loop(&self, bits: ControlState) -> Result<(), P64Error> {
        self.lua.call_loop(bits)
    }

    pub fn fail_instance(&mut self) {
        println!("shutdown the instance {} because lua thread ended", self.id);
    }

    pub fn call_main(&self) -> Result<(), P64Error> {
        self.lua.call_main()?;
        self.lua.call_loop(ControlState::default())
    }

    pub fn shutdown(&mut self) -> Result<(), P64Error> {
        let res = self.lua.die();
        self.lua.close();
        res
    }

    pub fn resize(&self, width: u32, height: u32) {
        self.lua.resize(width, height);
    }
}

pub struct BundleManager {
    pub console_bundle_target: u8,
    pub bundle_counter: u8,
    pub bundles: FxHashMap<u8, Bundle>,
    // #[cfg(feature = "headed")]
    // pub open_tex_managers: Vec<TexManager>,
    // pub open_lua_box: Vec<LuaCore>,
    pub call_order: Vec<u8>,
    main_rasters: Vec<Rc<RefCell<RgbaImage>>>,
    sky_rasters: Vec<Rc<RefCell<RgbaImage>>>,
}

impl BundleManager {
    pub fn new() -> Self {
        Self {
            console_bundle_target: 0,
            bundle_counter: 0,
            bundles: FxHashMap::default(),
            // #[cfg(feature = "headed")]
            // open_tex_managers: Vec::new(),
            // open_lua_box: Vec::new(),
            call_order: Vec::new(),
            main_rasters: Vec::new(),
            sky_rasters: Vec::new(),
        }
    }

    pub fn is_single(&self) -> bool {
        self.bundles.len() == 1
    }

    /** send resize event to each active bundle */
    pub fn resize(&mut self, width: u32, height: u32) {
        for bundle in self.bundles.values_mut() {
            bundle.resize(width, height);
            bundle.rasters.clear();
        }
        self.rebuild_call_order();
    }

    pub fn call_loop(&mut self, updated_bundles: &mut FxHashMap<u8, bool>, bits: &ControlState) {
        // An overlay takes input outright: it's an editor sitting on top of the app,
        // and letting keystrokes reach the game underneath would be both confusing
        // and destructive (typing into a source editor would also be driving the
        // game). Everyone else runs on a neutral snapshot, which is the same trick
        // the console already uses to keep its typing out of the game.
        let owner = self.input_owner();
        let quiet = ControlState::default();
        for (id, bundle) in &mut self.bundles.iter_mut() {
            let bits = if *id == owner { bits } else { &quiet };
            if !if let Some(updated) = updated_bundles.get_mut(id) {
                if *updated {
                    // frame_split is a divisor, so 1 means "every frame" — that needs
                    // zero skipped frames, not one. Comparing bare `skips >= frame_split`
                    // demanded a skip before *every* run, halving every bundle's tick
                    // rate to ~30Hz against a 60fps render.
                    if bundle.skips >= bundle.frame_split.saturating_sub(1) {
                        *updated = false;
                        bundle.skips = 0;
                        if (match bundle.skipped_control_state {
                            Some(old_bits) => {
                                bundle.skipped_control_state = None;
                                bundle.call_loop(combine_states(old_bits, *bits))
                            }
                            None => bundle.call_loop(*bits),
                        })
                        .is_err()
                        {
                            bundle.fail_instance();
                        }
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            } {
                //skip
                match bundle.skipped_control_state {
                    Some(old_bits) => {
                        bundle.skipped_control_state = Some(combine_states(old_bits, *bits));
                    }
                    None => {
                        bundle.skipped_control_state = Some(*bits);
                    }
                };
                // TODO overflow if we go over u16!!!
                bundle.skips += 1;
            }
        }
    }

    pub fn call_main(&mut self, bundle_id: u8) -> Result<(), P64Error> {
        if let Some(bundle) = self.bundles.get(&bundle_id) {
            return bundle.call_main();
        };
        Ok(())
    }

    pub fn make_bundle(
        &mut self,
        name: Option<&str>,
        bundle_relations: Option<(u8, bool)>,
        game_path: Option<&str>,
    ) -> &mut Bundle {
        let id = self.bundle_counter;
        self.bundle_counter += 1;
        // let gui = core.gui.make_morsel();
        // let tex_manager = crate::texture::TexManager::new();
        let lua = crate::lua_define::LuaCore::new();

        // find first index of emtpy slot
        let name = match name {
            Some(name) => name.to_string(),
            None => format!("local{}", id),
        };

        let mut bundle = Bundle::new(id, name, lua, game_path.map(|x| x.to_string()));
        match bundle_relations {
            Some((target, is_parent)) => {
                if is_parent {
                    match self.bundles.get_mut(&target) {
                        Some(target_bundle) => {
                            target_bundle.children.push(id);
                        }
                        _ => {}
                    };
                } else {
                    bundle.children.push(target);
                }
            }
            _ => {}
        }
        self.bundles.insert(id, bundle);
        self.rebuild_call_order();
        self.bundles.get_mut(&id).unwrap()
    }

    /// Which bundle receives real input this frame: the topmost overlay if any is up,
    /// otherwise the app itself.
    ///
    /// "Topmost" follows `call_order`, so it matches what the gui layers show — the
    /// surface you can see on top is the one you're typing into.
    pub fn input_owner(&self) -> u8 {
        for id in self.call_order.iter().rev() {
            if self.bundles.get(id).map_or(false, |b| b.overlay) {
                return *id;
            }
        }
        *self.call_order.first().unwrap_or(&0)
    }

    /// Bundles in gui-layer order: the app first, then overlays above it.
    ///
    /// The renderer reads this every frame rather than being told when to rewire, so
    /// loading or closing an overlay needs no bookkeeping and can't leave a layer
    /// pointing at a bundle that no longer exists.
    pub fn layer_order(&self) -> Vec<u8> {
        let mut out: Vec<u8> = self
            .call_order
            .iter()
            .copied()
            .filter(|id| self.bundles.get(id).map_or(false, |b| !b.overlay))
            .collect();
        out.extend(
            self.call_order
                .iter()
                .copied()
                .filter(|id| self.bundles.get(id).map_or(false, |b| b.overlay)),
        );
        out
    }

    /// Mark a bundle as an overlay. Engine-only by construction: this is not reachable
    /// from Lua.
    pub fn mark_overlay(&mut self, id: u8) {
        if let Some(b) = self.bundles.get_mut(&id) {
            b.overlay = true;
        }
    }

    pub fn rebuild_call_order(&mut self) {
        // Sorted, because this was seeded straight from `FxHashMap::keys()` — whose
        // order is arbitrary. That decided the order bundles run their Lua loops in,
        // and now also which gui layer each one draws to and which one owns input, so
        // an overlay could land on a different layer or miss input between runs with
        // nothing in the code having changed. Ids increase with creation, so ascending
        // order means the app comes first and overlays stack in the order they were
        // opened — the newest on top, which is what "topmost" should mean.
        let mut seed: Vec<u8> = self.bundles.keys().copied().collect();
        seed.sort_unstable();
        self.call_order = seed;

        for (bi, (k, b)) in self.bundles.iter().enumerate() {
            if !b.children.is_empty() {
                let mut i = self.call_order.iter().position(|x| *x == *k).unwrap();
                b.children
                    .iter()
                    .for_each(|c| match self.call_order.iter().position(|x| x == c) {
                        Some(x) => {
                            if x < i {
                                self.call_order.remove(i);
                                self.call_order.insert(x, *k);
                                i = x;
                            }
                        }
                        _ => {}
                    });
                self.call_order.extend(b.children.iter().cloned());
            }
        }
        // self.call_order.reverse();
        println!("bundles listed {}", self.list_bundles());
        println!("call order: {:?}", self.call_order);
        self.main_rasters.clear();
        self.sky_rasters.clear();
        for bi in self.call_order.iter() {
            if let Some(b) = self.bundles.get(bi) {
                println!("bundle {} has {} rasters", b.name, b.rasters.len());
                if let Some(r) = b.rasters.get(&0) {
                    self.main_rasters.push(r.clone());
                }
                if let Some(r) = b.rasters.get(&1) {
                    self.sky_rasters.push(r.clone());
                }
            }
        }
    }

    pub fn set_raster(&mut self, bundle_id: u8, raster: usize, im: RgbaImage) {
        // println!("set raster {} {}", bundle_id, raster);
        if let Some(b) = self.bundles.get_mut(&bundle_id) {
            match b.rasters.get_mut(&raster) {
                Some(r) => {
                    *r.borrow_mut() = im;
                }
                _ => {
                    b.rasters.insert(raster, Rc::new(RefCell::new(im)));
                    self.rebuild_call_order();
                }
            }
        }
    }

    pub fn get_rasters(&self, raster_id: usize) -> Option<RgbaImage> {
        if raster_id == 0 {
            // println!("1main raster {}", self.main_rasters.len());
            if self.main_rasters.len() > 0 {
                let im = self.main_rasters[0].borrow().clone();
                // TODO  raster overlay?
                println!("build up main raster {:?}", im.dimensions());
                // for r in self.main_rasters.iter().skip(1) {
                // image::imageops::overlay(&mut im, &r.borrow().clone(), 0, 0);
                // }
                // println!("2main raster {}", self.main_rasters.len());
                Some(im)
            } else {
                None
            }
        } else {
            if self.sky_rasters.len() > 0 {
                let im = self.sky_rasters[0].borrow().clone();
                // for r in self.sky_rasters.iter().skip(1) {
                //     image::imageops::overlay(&mut im, &r.borrow().clone(), 0, 0);
                // }
                println!("build up sky raster {:?}", im.dimensions());

                Some(im)
            } else {
                None
            }
        }
    }

    // pub fn get_bundle_raster(&self, bundle_id: u8, raster: usize) -> Option<&RgbaImage> {
    //     match self.bundles.get(&bundle_id) {
    //         Some(bundle) => match bundle.rasters.get(raster) {
    //             Some(raster) => Some(raster),
    //             None => None,
    //         },
    //         None => None,
    //     }
    // }

    pub fn get_lua(&mut self) -> &LuaCore {
        &self.get_main_bundle().lua
        // &self.bundles.get(&0).unwrap().lua
    }

    /// True if any bundle (game) is loaded. Callers should check this before
    /// routing input to `get_lua()`/`get_main_bundle()`, which panic when empty.
    pub fn has_bundles(&self) -> bool {
        !self.bundles.is_empty()
    }

    pub fn get_main_bundle(&mut self) -> &Bundle {
        match self.bundles.get(&self.console_bundle_target) {
            Some(bundle) => &bundle,
            None => {
                if self.bundles.len() > 0 {
                    let bundle = self.bundles.values().next().unwrap();
                    self.console_bundle_target = bundle.id;
                    &bundle
                } else {
                    panic!("No bundles loaded!");
                }
            }
        }
    }

    pub fn list_bundles(&self) -> String {
        self.bundles
            .iter()
            .map(|(key, val)| format!("{}->{}", key, val.name))
            .join(",")
    }

    /** Reset a specific bundle, returns true if it exists, and returns any possible children instances */
    pub fn soft_reset(&mut self, id: u8) -> (bool, Vec<u8>) {
        println!("call soft reset");
        match self.bundles.get_mut(&id) {
            Some(bundle) => {
                bundle.shutdown();
                (true, bundle.children.clone())
            }
            None => (false, vec![]),
        }
    }

    pub fn get(&self, index: u8) -> Option<&Bundle> {
        self.bundles.get(&index)
    }

    pub fn set_img_refs(&mut self, bundle_id: u8, main_ref: WeakWrapper, sky_ref: WeakWrapper) {
        if let Some(bundle) = self.bundles.get_mut(&bundle_id) {
            if let Some(pool) = &mut bundle.pool {
                pool.set_img_refs(main_ref, sky_ref);
            }
        }
    }

    pub fn get_pool(&self, index: u8) -> Option<&SharedPool> {
        match self.bundles.get(&index) {
            Some(bundle) => bundle.pool.as_ref(),
            None => None,
        }
    }

    /// Shut down and drop every overlay, leaving the app running.
    ///
    /// Separate from `soft_reset`, which resets a bundle *and its children*: an
    /// overlay is deliberately not a child of the app it edits, so reloading the app
    /// doesn't take the editor down with it.
    pub fn close_overlays(&mut self) -> usize {
        let ids: Vec<u8> = self
            .bundles
            .iter()
            .filter(|(_, b)| b.overlay)
            .map(|(id, _)| *id)
            .collect();
        for id in &ids {
            if let Some(mut b) = self.bundles.remove(id) {
                if let Err(e) = b.shutdown() {
                    eprintln!("failed to shut down overlay {}: {}", id, e);
                }
            }
        }
        if !ids.is_empty() {
            self.rebuild_call_order();
        }
        ids.len()
    }

    pub fn hard_reset(&mut self) {
        for (id, mut bundle) in self.bundles.drain() {
            if let Err(e) = bundle.shutdown() {
                eprintln!("failed to shutdown bundle  {} due to: {}", id, e);
            }
        }
        self.bundle_counter = 0;
        self.console_bundle_target = 0;
    }
    pub fn reclaim_resources(&mut self, _lua_returns: BundleResources) {}

    pub fn stats(&self) -> String {
        format!(
            "bundles: {} call order: {}, main rasters: {}, sky rasters: {}",
            self.bundles.len(),
            self.call_order.len(),
            self.main_rasters.len(),
            self.sky_rasters.len()
        )
    }
}

fn combine_states(mut old: ControlState, bits: ControlState) -> ControlState {
    for (i, key) in bits.0.iter().enumerate() {
        old.0[i] = old.0[i] || *key;
    }
    // replace first 2, add the next 2, 4 5 6 ||
    old.1[0] = bits.1[0];
    old.1[1] = bits.1[1];
    old.1[2] += bits.1[2];
    old.1[3] += bits.1[3];

    old.1[4] = (old.1[4] + bits.1[4]).clamp(0.0, 1.0);
    old.1[5] = (old.1[5] + bits.1[5]).clamp(0.0, 1.0);
    old.1[6] = (old.1[6] + bits.1[6]).clamp(0.0, 1.0);
    // old.1[7] += bits.1[7];

    old
}

pub struct BundleMutations {
    pub gui: bool,
    pub sky: bool,
}
impl BundleMutations {
    pub fn new() -> Self {
        Self {
            gui: true,
            sky: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bundle with no Lua context attached. `LuaCore::new()` allocates nothing and
    /// starts no thread, so the ordering rules can be tested for real rather than
    /// reimplemented in the test.
    fn app(bm: &mut BundleManager, name: &str) -> u8 {
        bm.make_bundle(Some(name), None, None).id
    }

    /// The surface on top owns input. With nothing overlaid that's the app; once an
    /// overlay is up it takes input outright, and the app must not also receive it —
    /// otherwise typing into an editor would simultaneously drive the game.
    #[test]
    fn input_goes_to_the_topmost_overlay() {
        let mut bm = BundleManager::new();
        let game = app(&mut bm, "game");
        assert_eq!(bm.input_owner(), game, "no overlay: the app owns input");

        let tool = app(&mut bm, "tool");
        bm.mark_overlay(tool);
        assert_eq!(bm.input_owner(), tool);

        // A second overlay stacks above the first and takes over.
        let tool2 = app(&mut bm, "tool2");
        bm.mark_overlay(tool2);
        assert_eq!(bm.input_owner(), tool2);
    }

    /// Layer assignment: the app draws to primary, overlays stack above it. Order
    /// matters — it's what decides which surface you see, and therefore which one
    /// `input_owner` hands input to.
    #[test]
    fn layer_order_puts_app_first_then_overlays() {
        let mut bm = BundleManager::new();
        let game = app(&mut bm, "game");
        let tool = app(&mut bm, "tool");
        bm.mark_overlay(tool);

        assert_eq!(bm.layer_order(), vec![game, tool]);

        // Even though this one has a lower id than an existing overlay would suggest,
        // non-overlays always come first: the app can't be pushed off primary.
        let tool2 = app(&mut bm, "tool2");
        bm.mark_overlay(tool2);
        assert_eq!(bm.layer_order(), vec![game, tool, tool2]);
    }

    /// Closing overlays leaves the app alone. They're deliberately not children of
    /// the app, so neither teardown direction should take the other with it.
    #[test]
    fn close_overlays_leaves_the_app_running() {
        let mut bm = BundleManager::new();
        let game = app(&mut bm, "game");
        let tool = app(&mut bm, "tool");
        bm.mark_overlay(tool);

        assert_eq!(bm.close_overlays(), 1);
        assert!(bm.get(game).is_some(), "the app must survive");
        assert!(bm.get(tool).is_none(), "the overlay must be gone");
        assert_eq!(bm.input_owner(), game, "input returns to the app");
        assert_eq!(bm.layer_order(), vec![game]);
        assert_eq!(bm.close_overlays(), 0, "closing again is a no-op");
    }

    /// Drive `call_loop` for `frames`, pretending Lua reports its loop complete before
    /// each one, and count how many frames actually ran. A run is visible as the
    /// updated flag being consumed.
    fn ticks_over(bm: &mut BundleManager, id: u8, frames: usize) -> usize {
        let bits = ControlState::default();
        let mut ran = 0;
        for _ in 0..frames {
            let mut updated = FxHashMap::default();
            updated.insert(id, true);
            bm.call_loop(&mut updated, &bits);
            if !updated[&id] {
                ran += 1;
            }
        }
        ran
    }

    /// `frame_split` is a divisor, so the default of 1 has to tick on every frame.
    /// The gate used to read `skips >= frame_split`, which demanded a skipped frame
    /// before *every* run and quietly halved every bundle to ~30Hz under a 60fps
    /// render — including overlays, where an editor tick that slow is very visible.
    #[test]
    fn frame_split_of_one_ticks_every_frame() {
        let mut bm = BundleManager::new();
        let game = app(&mut bm, "game");
        assert_eq!(bm.get(game).unwrap().frame_split, 1, "the default");

        assert_eq!(ticks_over(&mut bm, game, 10), 10);
    }

    /// Splitting still works: it's the only reason the counter exists.
    #[test]
    fn frame_split_of_two_ticks_every_other_frame() {
        let mut bm = BundleManager::new();
        let game = app(&mut bm, "game");
        bm.bundles.get_mut(&game).unwrap().frame_split = 2;

        assert_eq!(ticks_over(&mut bm, game, 10), 5);
    }
}

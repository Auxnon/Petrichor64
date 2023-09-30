use std::{cell::RefCell, rc::Rc, sync::mpsc::Receiver};

use image::RgbaImage;
use itertools::Itertools;
use rustc_hash::FxHashMap;

#[cfg(feature = "headed")]
use crate::root::Core;
// use crate::texture::TexManager;
use crate::{
    gui::PreGuiMorsel,
    lua_define::{LuaCore, LuaHandle, MainPacket},
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
    /** How many frames do we want to intentionally skip to bring our fps down? Not great */
    pub frame_split: u16,
    pub lua_ctx_handle: Option<LuaHandle>,
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
        }
    }

    pub fn get_directory(&self) -> Option<&str> {
        self.directory.as_deref()
    }

    pub fn call_loop(&self, bits: ControlState) {
        self.lua.call_loop(bits);
    }

    pub fn call_main(&self) {
        self.lua.call_main();
        self.lua.call_loop(([false; 256], [0.; 11]));
    }

    pub fn shutdown(&self) {
        self.lua.die();
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
    pub open_lua_box: Vec<LuaCore>,
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
            open_lua_box: Vec::new(),
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

    pub fn call_loop(&mut self, updated_bundles: &mut FxHashMap<u8, bool>, bits: ControlState) {
        for (id, bundle) in &mut self.bundles.iter_mut() {
            if !if let Some(updated) = updated_bundles.get_mut(id) {
                if *updated {
                    if bundle.skips >= bundle.frame_split {
                        *updated = false;
                        bundle.skips = 0;
                        match bundle.skipped_control_state {
                            Some(old_bits) => {
                                bundle.call_loop(combine_states(old_bits, bits));
                                bundle.skipped_control_state = None;
                            }
                            None => {
                                bundle.call_loop(bits);
                            }
                        }
                        bundle.call_loop(bits);
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
                        bundle.skipped_control_state = Some(combine_states(old_bits, bits));
                    }
                    None => {
                        bundle.skipped_control_state = Some(bits);
                    }
                };
                bundle.skips += 1;
            }
        }
    }

    pub fn call_main(&mut self, bundle_id: u8) {
        match self.bundles.get(&bundle_id) {
            Some(bundle) => bundle.call_main(),
            None => {}
        };
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

    pub fn rebuild_call_order(&mut self) {
        self.call_order = self.bundles.keys().copied().collect();

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
                let mut im = self.main_rasters[0].borrow().clone();
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
                let mut im = self.sky_rasters[0].borrow().clone();
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
    pub fn soft_reset(&self, id: u8) -> (bool, Vec<u8>) {
        match self.bundles.get(&id) {
            Some(bundle) => {
                bundle.shutdown();
                (true, bundle.children.clone())
            }
            None => (false, vec![]),
        }
    }

    pub fn hard_reset(&mut self) {
        for (id, bundle) in self.bundles.drain() {
            bundle.shutdown();
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

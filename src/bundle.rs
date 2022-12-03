use std::{cell::RefCell, rc::Rc};

use image::RgbaImage;
use itertools::Itertools;
use rustc_hash::FxHashMap;

use crate::{
    controls::ControlState, gui::GuiMorsel, lua_define::LuaCore, texture::TexManager, Core,
};

/**
 * Represent a bundle of scripts and assets occupying a single lua instance or game.
 */

pub struct Bundle {
    pub id: u8,
    pub name: String,
    pub directory: Option<String>,
    pub lua: LuaCore,
    pub children: Vec<u8>,
    pub rasters: FxHashMap<usize, Rc<RefCell<RgbaImage>>>,
}

pub type BundleResources = GuiMorsel;

impl Bundle {
    pub fn new(id: u8, name: String, lua: LuaCore) -> Self {
        Self {
            id,
            name,
            directory: None,
            lua,
            children: Vec::new(),
            rasters: FxHashMap::default(),
        }
    }

    pub fn call_loop(&self, bits: ControlState) {
        self.lua.call_loop(bits);
    }
    pub fn call_main(&self) {
        self.lua.call_main()
    }
    pub fn shutdown(&self) {
        self.lua.die();
    }
}

pub struct BundleManager {
    pub console_bundle_target: u8,
    pub bundle_counter: u8,
    pub bundles: FxHashMap<u8, Bundle>,
    pub open_tex_managers: Vec<TexManager>,
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
            open_tex_managers: Vec::new(),
            open_lua_box: Vec::new(),
            call_order: Vec::new(),
            main_rasters: Vec::new(),
            sky_rasters: Vec::new(),
        }
    }

    pub fn is_single(&self) -> bool {
        self.bundles.len() == 1
    }

    pub fn call_loop(&mut self, bits: ControlState) {
        for bundle in &mut self.bundles.values() {
            bundle.call_loop(bits);
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
        name: Option<String>,
        bundle_relations: Option<(u8, bool)>,
    ) -> &mut Bundle {
        let id = self.bundle_counter;
        self.bundle_counter += 1;
        // let gui = core.gui.make_morsel();
        // let tex_manager = crate::texture::TexManager::new();
        let lua = crate::lua_define::LuaCore::new();

        // find first index of emtpy slot
        // self.bundles.
        let mut bundle = Bundle::new(id, name.unwrap_or(format!("local{}", id)), lua);
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

    // pub fn make_and_start_bundle(&mut self, name: Option<String>, core: &Core) -> &mut Bundle {
    //     let bundle = self.make_bundle(name);
    //     let resources = core.gui.make_morsel();
    //     let lua_handle = bundle.lua.start(
    //         bundle.id,
    //         resources,
    //         core.world.sender.clone(),
    //         core.pitcher.clone(),
    //         core.singer.clone(),
    //         false,
    //     );
    //     bundle
    // }

    // pub fn start_bundle(resources: BundleResources, bundle: &mut Bundle) {
    //     // let resources = core.gui.make_morsel();
    //     let lua_handle = bundle.lua.start(
    //         bundle.id,
    //         resources,
    //         core.world.sender.clone(),
    //         core.pitcher.clone(),
    //         core.singer.clone(),
    //         false,
    //     );
    // }
    pub fn provide_resources(core: &Core) -> BundleResources {
        core.gui.make_morsel()
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
                for r in self.main_rasters.iter().skip(1) {
                    // image::imageops::overlay(&mut im, &r.borrow().clone(), 0, 0);
                }
                // println!("2main raster {}", self.main_rasters.len());
                Some(im)
            } else {
                None
            }
        } else {
            if self.sky_rasters.len() > 0 {
                let mut im = self.sky_rasters[0].borrow().clone();
                for r in self.sky_rasters.iter().skip(1) {
                    image::imageops::overlay(&mut im, &r.borrow().clone(), 0, 0);
                }
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
        match self.bundles.get(&self.console_bundle_target) {
            Some(bundle) => &bundle.lua,
            None => {
                if self.bundles.len() > 0 {
                    let bundle = self.bundles.values().next().unwrap();
                    self.console_bundle_target = bundle.id;
                    &bundle.lua
                } else {
                    panic!("No bundles loaded!");
                }
            }
        }

        // &self.bundles.get(&0).unwrap().lua
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

    // pub fn get_bundle(&self, id: u8) -> Option<&Bundle> {
    //     self.bundles.get(id as usize)
    // }
}

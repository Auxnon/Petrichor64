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
        }
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
        println!("bundles listed {}", self.list_bundles());
        println!("call order: {:?}", self.call_order);
    }

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

    /** Reset a specific bundle, returns true if it exists */
    pub fn soft_reset(&self, id: u8) -> bool {
        match self.bundles.get(&id) {
            Some(bundle) => {
                bundle.shutdown();
                true
            }
            None => false,
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

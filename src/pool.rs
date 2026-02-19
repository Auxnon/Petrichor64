use std::{cell::RefCell, rc::Rc, sync::Arc};

use atomicell::AtomicCell;
use image::RgbaImage;
use silt_lua::userdata::WeakWrapper;

use crate::lua_ent::LuaEnt;

pub type Shared<'a, T> = Rc<RefCell<Option<atomicell::Ref<'a, T>>>>;
pub type SharedMut<'a, T> = Rc<RefCell<Option<atomicell::RefMut<'a, T>>>>;
pub type AtomicImage = Arc<AtomicCell<RgbaImage>>;
pub struct SharedPool {
    pub ent_list: Arc<AtomicCell<Vec<LuaEnt>>>,
    pub gui: Option<WeakWrapper>,
    pub sky: Option<WeakWrapper>,
    pub gui_dirty: Arc<AtomicCell<bool>>,
    pub sky_dirty: Arc<AtomicCell<bool>>,
}

impl SharedPool {
    pub fn new(gui: AtomicImage, sky: AtomicImage) -> Self {
        Self {
            ent_list: Arc::new(AtomicCell::new(Vec::new())),
            gui: None,
            sky: None,
            gui_dirty: Arc::new(AtomicCell::new(false)),
            sky_dirty: Arc::new(AtomicCell::new(false)),
        }
    }

    pub fn set_img_refs(&mut self, gui: WeakWrapper, sky: WeakWrapper) {
        self.gui = Some(gui);
        self.sky = Some(sky);
    }

    pub fn localize<'a>(&'a self, local: &mut LocalPool<'a>) -> Result<(), ()> {
        if let Some(el) = self.ent_list.try_borrow_mut() {
            local.ent_list.replace(Some(el));
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn clone(&self) -> Self {
        Self {
            ent_list: self.ent_list.clone(),
            gui: None,
            sky: None,
            gui_dirty: self.gui_dirty.clone(),
            sky_dirty: self.sky_dirty.clone(),
        }
    }
}

impl Clone for SharedPool {
    fn clone(&self) -> Self {
        Self {
            ent_list: self.ent_list.clone(),
            gui: None,
            sky: None,
            gui_dirty: self.gui_dirty.clone(),
            sky_dirty: self.sky_dirty.clone(),
        }
    }
}

pub struct LocalPool<'a> {
    pub active: bool,
    pub ent_list: SharedMut<'a, Vec<LuaEnt>>,
}

impl<'a> LocalPool<'a> {
    pub fn new() -> Self {
        Self {
            active: false,
            ent_list: Rc::new(RefCell::new(None)),
        }
    }

    pub fn drop(&mut self) {
        self.active = false;
        self.ent_list.replace(None);
    }

    pub fn check_lock(&mut self, pool: &'a SharedPool) {
        if !self.active {
            self.active = true;
            pool.localize(self);
        }
    }
}

impl Clone for LocalPool<'_> {
    fn clone(&self) -> Self {
        Self {
            active: false, // not the controller so we can ignore this in additional clones
            ent_list: self.ent_list.clone(),
        }
    }
}

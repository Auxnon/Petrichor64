use std::{cell::RefCell, rc::Rc, sync::Arc};

use atomicell::AtomicCell;
use image::RgbaImage;

use crate::{ent::Ent, lua_ent::LuaEnt};

pub type Shared<'a, T> = Rc<RefCell<Option<atomicell::Ref<'a, T>>>>;
pub type SharedMut<'a, T> = Rc<RefCell<Option<atomicell::RefMut<'a, T>>>>;
pub type AtomicImage = Arc<AtomicCell<RgbaImage>>;
pub struct SharedPool {
    pub ent_list: Arc<AtomicCell<Vec<LuaEnt>>>,
    pub gui: Arc<AtomicCell<RgbaImage>>,
    pub sky: Arc<AtomicCell<RgbaImage>>,
}

impl SharedPool {
    pub fn new(gui: AtomicImage, sky: AtomicImage) -> Self {
        Self {
            ent_list: Arc::new(AtomicCell::new(Vec::new())),
            gui,
            sky,
        }
    }

    pub fn localize<'a>(&'a self, local: &mut LocalPool<'a>) -> Result<(), ()> {
        if let (Ok(el), Ok(g), Ok(s)) = (
            self.ent_list.try_borrow_mut(),
            self.gui.try_borrow_mut(),
            self.sky.try_borrow_mut(),
        ) {
            local.ent_list.replace(Some(el));
            local.gui.replace(Some(g));
            local.sky.replace(Some(s));
            Ok(())
        } else {
            Err(())
        }
    }

    pub fn clone(&self) -> Self {
        Self {
            ent_list: self.ent_list.clone(),
            gui: self.gui.clone(),
            sky: self.sky.clone(),
        }
    }

    // pub fn make(&mut self) {
    //     // AtomicCell::is_lock_free()
    //     let b = self.ent_list.borrow();
    //     let a = self.ent_list.borrow_mut();
    // }
}

impl Clone for SharedPool {
    fn clone(&self) -> Self {
        Self {
            ent_list: self.ent_list.clone(),
            gui: self.gui.clone(),
            sky: self.sky.clone(),
        }
    }
}

pub struct LocalPool<'a> {
    pub active: bool,
    pub ent_list: SharedMut<'a, Vec<LuaEnt>>,
    pub gui: SharedMut<'a, RgbaImage>,
    pub sky: SharedMut<'a, RgbaImage>,
}

impl<'a> LocalPool<'a> {
    pub fn new() -> Self {
        Self {
            active: false,
            ent_list: Rc::new(RefCell::new(None)),
            gui: Rc::new(RefCell::new(None)),
            sky: Rc::new(RefCell::new(None)),
        }
    }

    pub fn drop(&mut self) {
        self.active = false;
        self.ent_list.replace(None);
        self.gui.replace(None);
        self.sky.replace(None);
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
            gui: self.gui.clone(),
            sky: self.sky.clone(),
        }
    }
}

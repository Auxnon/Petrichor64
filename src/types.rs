#[cfg(feature = "silt")]
#[allow(unused_imports)]
use crate::silt_str::{lstr, TableStr};
#[cfg(feature = "silt")]
use silt_lua::{
    gc_arena::Mutation,
    prelude::Table,
    VM,
};

#[cfg(feature = "puc_lua")]
use mlua::{prelude::LuaError, Table};

#[cfg(feature = "picc")]
use piccolo::{Context, InvalidTableKey, Table};

pub enum ValueMap {
    String(String),
    Integer(i32),
    Float(f32),
    Bool(bool),
    // Table(Vec<(String, ValueMap)>),
    Array(Vec<ValueMap>),
    // Map(std::collections::HashMap<String, ValueMap>),
    Null(),
}

/// Where the touches start in `ControlState`'s analog array: the first 11 are
/// the mouse (position, delta, buttons, scroll, unprojected cursor).
pub const ANALOG_TOUCH: usize = 11;
/// The analog array's length: the mouse, then the touch slots (`pack_touches`).
pub const ANALOG_LEN: usize = ANALOG_TOUCH + crate::global::TOUCH_FLOATS * crate::global::TOUCH_SLOTS;

/// The script side's copy of the pointer (`mus()`, `touches()`): the 13 mouse
/// values (with last frame's position as px, py), then the touch slots.
pub const MICE_LEN: usize = 13 + crate::global::TOUCH_FLOATS * crate::global::TOUCH_SLOTS;

#[derive(Clone, Copy)]
pub struct ControlState(pub [bool; 256], pub [f32; ANALOG_LEN]);

impl Default for ControlState {
    fn default() -> Self {
        Self([false; 256], [0.; ANALOG_LEN])
    }
}

pub struct GlobalMap {
    pub os: &'static str,
    pub hertz: f32,
    pub resolution: (u32, u32),
    /// Window size in physical pixels. The 3D view's aspect is `window.0 / window.1`
    /// (the projection is built from the surface size), which a script needs to
    /// place anything relative to the screen edges in 3D.
    pub window: (u32, u32),
    /// The gui raster's actual size — `resolution` stretched to the window's aspect.
    pub gui: (u32, u32),
    /// Window pixels the system covers at each edge: top, right, bottom, left.
    pub inset: [f32; 4],
}

impl GlobalMap {
    pub fn new(os: &'static str, hertz: f32, res: (u32, u32)) -> Self {
        Self {
            os,
            hertz,
            resolution: res,
            window: res,
            gui: res,
            inset: [0.; 4],
        }
    }

    #[allow(dead_code)] // headed builds only
    pub fn with_inset(mut self, inset: [f32; 4]) -> Self {
        self.inset = inset;
        self
    }

    pub fn with_screen(mut self, window: (u32, u32), gui: (u32, u32)) -> Self {
        self.window = (window.0.max(1), window.1.max(1));
        self.gui = gui;
        self
    }

    #[cfg(feature = "picc")]
    pub fn convert<'gc>(
        &self,
        ctx: &Context<'gc>,
        table: &mut Table<'gc>,
    ) -> Result<(), InvalidTableKey> {
        let c = *ctx;
        table.set(c, "os", self.os)?;
        table.set(c, "hz", self.hertz)?;
        table.set(c, "res", [self.resolution.0, self.resolution.1])?;
        Ok(())
    }

    #[cfg(feature = "puc_lua")]
    pub fn convert(&self, table: &mut Table) -> Result<(), LuaError> {
        table.set("os", self.os)?;
        table.set("hz", self.hertz)?;
        table.set("res", [self.resolution.0, self.resolution.1])?;
        Ok(())
    }

    #[cfg(feature = "silt")]
    pub fn convert<'gc>(&self, vm: &mut VM<'gc>, mc: &Mutation<'gc>, table: &mut Table<'gc>) {
        table.set_str(mc, "os", lstr(mc, self.os));
        table.set_str(mc, "hz", self.hertz);

        let mut res_tbl = vm.raw_table();
        res_tbl.set(0, self.resolution.0);
        res_tbl.set(1, self.resolution.1);
        table.set_str(mc, "res", vm.wrap_table(mc, res_tbl));
        table.set_str(mc, "w", self.window.0);
        table.set_str(mc, "h", self.window.1);
        table.set_str(mc, "aspect", self.window.0 as f64 / self.window.1 as f64);
        table.set_str(mc, "gw", self.gui.0);
        table.set_str(mc, "gh", self.gui.1);
        // the edges the system draws over, in the same pixels as w and h
        let mut inset = vm.raw_table();
        for (k, v) in ["top", "right", "bottom", "left"].iter().zip(self.inset) {
            inset.set_str(mc, k, v);
        }
        table.set_str(mc, "inset", vm.wrap_table(mc, inset));
    }
}

pub struct Script {
    pub name: String,
    pub content: String,
}

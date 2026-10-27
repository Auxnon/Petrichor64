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

#[derive(Clone, Copy)]
pub struct ControlState(pub [bool; 256], pub [f32; 11]);

impl Default for ControlState {
    fn default() -> Self {
        Self([false; 256], [0.; 11])
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
}

impl GlobalMap {
    pub fn new(os: &'static str, hertz: f32, res: (u32, u32)) -> Self {
        Self {
            os,
            hertz,
            resolution: res,
            window: res,
            gui: res,
        }
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
    }
}

pub struct Script {
    pub name: String,
    pub content: String,
}

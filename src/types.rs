#[cfg(feature = "silt")]
use silt_lua::prelude::{LuaError, Table};

#[cfg(feature = "puc_lua")]
use mlua::{prelude::LuaError, Table};

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

pub type ControlState = ([bool; 256], [f32; 11]);

pub struct GlobalMap {
    pub os: &'static str,
    pub hertz: f32,
    pub resolution: (u32, u32),
}

impl GlobalMap {
    pub fn new(os: &'static str, hertz: f32, res: (u32, u32)) -> Self {
        Self {
            os,
            hertz,
            resolution: res,
        }
    }
    pub fn convert(&self, table: &mut Table) -> Result<(), LuaError> {
        table.set("os", self.os)?;
        table.set("hz", self.hertz)?;
        table.set("res", [self.resolution.0, self.resolution.1])?;
        Ok(())
    }
}

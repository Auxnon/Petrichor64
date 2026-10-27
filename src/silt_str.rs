//! silt strings are garbage-collected (`Gc<LuaStr>`), so building one needs the
//! arena's `Mutation`, and `&str`/`String` no longer convert into a `Value` by
//! themselves. These helpers keep the engine's many string-keyed table reads and
//! writes short.

use silt_lua::{
    gc_arena::Mutation,
    lstr::LuaStr,
    prelude::{Table, Value},
};

/// A silt string value.
#[inline]
pub fn lstr<'gc>(mc: &Mutation<'gc>, s: impl Into<Box<str>>) -> Value<'gc> {
    Value::String(LuaStr::gc(mc, s))
}

pub trait TableStr<'gc> {
    /// `t[key] = val` with a string key. Returns the previous value.
    fn set_str(
        &mut self,
        mc: &Mutation<'gc>,
        key: &str,
        val: impl Into<Value<'gc>>,
    ) -> Option<Value<'gc>>;

    /// `t[key]` with a string key, `nil` when absent.
    fn str_value(&self, key: &str) -> Value<'gc>;
}

impl<'gc> TableStr<'gc> for Table<'gc> {
    #[inline]
    fn set_str(
        &mut self,
        mc: &Mutation<'gc>,
        key: &str,
        val: impl Into<Value<'gc>>,
    ) -> Option<Value<'gc>> {
        self.set(lstr(mc, key), val)
    }

    #[inline]
    fn str_value(&self, key: &str) -> Value<'gc> {
        self.get_str(key).copied().unwrap_or(Value::Nil)
    }
}

/// An owned string that becomes a silt string when it reaches the VM, for
/// places that convert later and have no `Mutation` at hand, such as the
/// arguments to `call_global`.
pub struct LStr(pub String);

impl<'a> silt_lua::value::ToLua<'a> for LStr {
    fn to_lua(
        self,
        _: &silt_lua::lua::VM<'a>,
        mc: &Mutation<'a>,
    ) -> Result<Value<'a>, silt_lua::LuaError> {
        Ok(lstr(mc, self.0))
    }
}

//! Lua value <-> JSON, for `connection:send(table)` / `connection:recv(true)`.
//!
//! silt tables are one hashmap, so a table counts as an array only when its keys
//! are exactly `1..=n`; anything else (including an empty table) becomes an
//! object with stringified keys. Decoded arrays are built with `push` so `#t`
//! works, and a JSON `null` inside an array still occupies its slot.

use silt_lua::gc_arena::Mutation;
use serde_json::{Map, Number, Value as Json};
use silt_lua::{prelude::Value, VM};

/// Deep enough for any sane payload, shallow enough to stop a cyclic table.
const MAX_DEPTH: usize = 64;

pub fn to_json(v: &Value) -> Result<Json, String> {
    encode(v, 0)
}

fn num(f: f64) -> Json {
    Number::from_f64(f).map(Json::Number).unwrap_or(Json::Null)
}

fn encode(v: &Value, depth: usize) -> Result<Json, String> {
    if depth > MAX_DEPTH {
        return Err("table nested too deep (cyclic?) to encode as json".to_owned());
    }
    Ok(match v {
        Value::Nil => Json::Null,
        Value::Bool(b) => Json::Bool(*b),
        Value::Integer(i) => Json::Number((*i).into()),
        Value::Number(f) => num(*f),
        Value::Infinity(_) => Json::Null,
        Value::String(s) => Json::String(s.to_string()),
        #[cfg(feature = "silt")]
        Value::Vec2(v) => Json::Array(vec![num(v.x as f64), num(v.y as f64)]),
        #[cfg(feature = "silt")]
        Value::Vec3(v) => Json::Array(vec![num(v.x as f64), num(v.y as f64), num(v.z as f64)]),
        #[cfg(feature = "silt")]
        Value::Vec4(v) => Json::Array(vec![
            num(v.x as f64),
            num(v.y as f64),
            num(v.z as f64),
            num(v.w as f64),
        ]),
        Value::Table(t) => {
            let t = t.borrow();
            let n = t.len();
            let is_array = n > 0 && (1..=n).all(|i| t.getn(i).is_some());
            if is_array {
                let mut arr = Vec::with_capacity(n);
                for i in 1..=n {
                    arr.push(encode(t.getn(i).unwrap(), depth + 1)?);
                }
                Json::Array(arr)
            } else {
                let mut map = Map::new();
                for (k, v) in t.iter() {
                    let key = match k {
                        Value::String(s) => s.to_string(),
                        Value::Integer(i) => i.to_string(),
                        Value::Number(f) => f.to_string(),
                        Value::Bool(b) => b.to_string(),
                        _ => continue,
                    };
                    map.insert(key, encode(v, depth + 1)?);
                }
                Json::Object(map)
            }
        }
        other => return Err(format!("cannot encode a {} as json", other.type_name())),
    })
}

pub fn from_json<'gc>(vm: &VM<'gc>, mc: &Mutation<'gc>, j: Json) -> Value<'gc> {
    match j {
        Json::Null => Value::Nil,
        Json::Bool(b) => Value::Bool(b),
        Json::Number(n) => match n.as_i64() {
            Some(i) => Value::Integer(i),
            None => Value::Number(n.as_f64().unwrap_or(0.)),
        },
        Json::String(s) => vm.new_string(mc, s),
        Json::Array(a) => {
            let mut t = vm.raw_table();
            for v in a {
                t.push(from_json(vm, mc, v));
            }
            vm.wrap_table(mc, t)
        }
        Json::Object(o) => {
            let mut t = vm.raw_table();
            for (k, v) in o {
                t.set(vm.new_string(mc, k), from_json(vm, mc, v));
            }
            vm.wrap_table(mc, t)
        }
    }
}

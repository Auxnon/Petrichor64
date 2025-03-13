use ron::de::from_reader;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::log::{LogType, Loggy};

#[derive(Deserialize, Default, Debug)]
pub struct AssetTemplate {
    #[serde(default = "default_tile_name")]
    pub name: String,
    #[serde(default)]
    pub tiles: HashMap<u32, String>,
    #[serde(default = "default_tile_size")]
    pub size: u32,
    #[serde(default = "default_anim")]
    pub anims: Vec<(String, Vec<String>, bool)>,
}

// #[derive(Debug)]
// pub struct TemplateAnim {
//     name: String,
//     anims: Vec<String>,
//     once: bool,
// }

// impl TemplateAnim {
//     pub fn new() -> TemplateAnim {
//         Self {
//             name: String::new(),
//             anims: vec![],
//             once: false,
//         }
//     }
// }

// impl std::fmt::Debug for Anim {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         f.debug_struct("Anim")
//             .field("name", &self.name)
//             .field("anims", &self.anims.join(","))
//             .finish()
//     }
// }

fn default_tile_name() -> String {
    "".to_string()
}

fn default_tile_size() -> u32 {
    0 as u32
}
fn default_anim() -> Vec<(String, Vec<String>, bool)> {
    vec![]
}

/** load file as a ron template for asset */
pub fn load_ron(path: &str, loggy: &mut Loggy) -> Option<AssetTemplate> {
    match std::fs::File::open(path) {
        Ok(f) => match from_reader(f) {
            Ok(x) => Some(x),
            Err(e) => {
                loggy.log(
                    LogType::ConfigError,
                    &format!("problem with template {}", e),
                );
                None
            }
        },
        _ => None,
    }
}

/** load file as a ron template for asset */
pub fn load_json(filename: &str, path: &str) -> Option<AssetTemplate> {
    match read_json_from_file(path) {
        Ok(str) => interpret_json(filename, &str),
        _ => None,
    }
}

/** interpret string as a ron template for asset */
pub fn interpret_ron(s: &str, loggy: &mut Loggy) -> Option<AssetTemplate> {
    match ron::from_str(s) {
        Ok(x) => Some(x),
        Err(e) => {
            loggy.log(
                LogType::ConfigError,
                &format!("problem with template {}", e),
            );
            //std::process::exit(1);
            None
        }
    }
}

fn read_json_from_file<P: AsRef<Path>>(path: P) -> Result<String, Box<dyn Error>> {
    // Open the file in read-only mode with buffer.
    let file = File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    // Read the JSON contents of the file as an instance of `User`.

    let mut line = String::new();
    // reader.read_to_end(buf)
    // reader.read_to_string(buf)
    reader.read_to_string(&mut line)?;

    // let u = serde_json::from_reader(reader)?;
    // // Return the `User`.
    // Ok(u)

    Ok(line)
}

/** interpret string as a json template for asset */
pub fn interpret_json(filename: &str, s: &str) -> Option<AssetTemplate> {
    match serde_json::from_str::<serde_json::Value>(s) {
        Ok(j) => process_json(filename, j),
        _ => None,
    }
}

/** inbetween json process function */
fn process_json(filename: &str, j: Value) -> Option<AssetTemplate> {
    // println!("🟣we got a json {}", j);
    let dim = match &j["frames"] {
        Value::Object(m) => match m.values().next() {
            Some(Value::Object(m2)) => {
                let frame = &m2["frame"];
                let w = match frame["w"].as_u64() {
                    Some(x) => x as u32,
                    _ => 16,
                };
                let h = match frame["h"].as_u64() {
                    Some(x) => x as u32,
                    _ => 16,
                };
                (w, h)
            }
            _ => (default_tile_size(), default_tile_size()),
        },
        _ => (default_tile_size(), default_tile_size()),
    };
    println!("🟣we got a json dim {} {}", dim.0, dim.1);

    let m = &j["meta"];
    if m.is_object() {
        let tags = &m["frameTags"];
        match tags.as_array() {
            Some(ar) => {
                let mut anims = vec![];
                for tag in ar {
                    // { "name": "Walk", "from": 0, "to": 6, "direction": "forward" },
                    match tag["name"].as_str() {
                        Some(name) => {
                            let from = &tag["from"];
                            let to = &tag["to"];
                            let direction = &tag["direction"];
                            let once = &tag["once"];

                            let ifrom = match from.as_u64() {
                                Some(i) => i,
                                None => 0,
                            };

                            let ito = match to.as_u64() {
                                Some(i) => i,
                                None => 0,
                            };

                            let idir = match direction.as_str() {
                                Some(i) => i,
                                None => "forward",
                            };

                            let ionce = match once.as_bool() {
                                Some(o) => o,
                                _ => false,
                            };

                            let frames: Vec<u64> = match idir {
                                "ping-pong" => {
                                    let mut v = (ifrom..ito + 1).collect::<Vec<u64>>();
                                    v.extend((ifrom..(ito)).rev());
                                    v
                                }
                                "reverse" => (ifrom..ito + 1).rev().collect(),
                                _ => (ifrom..ito + 1).collect(),
                            };

                            // println!(
                            //     "🟣we got a json anim {} {} {:?}",
                            //     name,
                            //     frames.len(),
                            //     frames
                            // );

                            anims.push((
                                name.to_string(),
                                frames
                                    .iter()
                                    .map(|n| format!("{}{}", filename, n))
                                    .collect(),
                                ionce,
                            ));
                        }
                        _ => {}
                    }
                }
                return Some(AssetTemplate {
                    name: default_tile_name(),
                    tiles: HashMap::new(),
                    size: dim.0,
                    anims,
                });
            }
            _ => {}
        }
    }
    None
}

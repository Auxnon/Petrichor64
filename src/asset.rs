use std::{
    fs::read_dir,
    path::{Path, PathBuf},
};

use wgpu::Device;

use crate::lua_define::LuaCore;

pub fn init(device: &Device) {
    let assets_path = Path::new(".").join("assets");
    let scripts_path = Path::new(".").join("scripts");
    log(format!("assets dir is {}", assets_path.display()));

    let assets_dir: Vec<PathBuf> = read_dir(&assets_path)
        .expect("Assets directory failed to load")
        .filter(Result::is_ok)
        .map(|e| e.unwrap().path())
        .collect();

    for entry in assets_dir {
        log(format!("asset to load {}", entry.display()));
        match entry.extension() {
            Some(e) => {
                let s = e.to_ascii_lowercase();
                let file_name = &entry.file_name().unwrap().to_str().unwrap().to_string();
                if s == "glb" || s == "gltf" {
                    log(format!("loading {} as glb/gltf model", file_name));
                    crate::model::load(file_name, device);
                } else if s == "png" {
                    log(format!("loading {} as png image", file_name));
                    crate::texture::load_tex(file_name.to_string());
                } else {
                    log(format!("unknown file type {}", file_name));
                }
            }
            None => {}
        }
        // let f = File::open(&entry).expect("Failed opening an entity file");
        // let schema: PreEntSchema = match from_reader(f) {
        //     Ok(x) => x,
        //     Err(e) => {
        //         println!("Failed to apply entity RON schema, defaulting: {}", e);
        //         //std::process::exit(1);
        //         PreEntSchema::default()
        //     }
        // };
    }

    log(format!("scripts dir is {}", scripts_path.display()));

    let scripts_dir: Vec<PathBuf> = read_dir(&scripts_path)
        .expect("Scripts directory failed to load")
        .filter(Result::is_ok)
        .map(|e| e.unwrap().path())
        .collect();
    for entry in scripts_dir {
        match entry.extension() {
            Some(e) => {
                let s = e.to_ascii_lowercase();
                let file_name = &entry.file_name().unwrap().to_str().unwrap().to_string();
                if s == "lua" {
                    log(format!("loading  script {}", file_name));

                    let path = Path::new("scripts")
                        .join(file_name.to_owned())
                        .with_extension("lua")
                        .to_str()
                        .unwrap()
                        .to_string();

                    let mutex = crate::lua_master.lock();
                    //log(format!("hooked {}", path));
                    let d = mutex.get().unwrap();

                    d.load(path);
                    //crate::model::load(file_name, device);
                } else {
                    log(format!("skipping file type {}", file_name));
                }
            }
            None => {}
        }
    }
}

pub fn get_file_name(str: String) -> String {
    let bits = str.split(".").collect::<Vec<_>>();
    match bits.get(0) {
        Some(o) => o.to_string(),
        None => str,
    }
}
pub fn get_file_file_name(str: String) -> String {
    let s = get_file_name(str);
    let bits = s.split("/").collect::<Vec<_>>();
    match bits.get(1) {
        Some(o) => o.to_string(),
        None => s,
    }
}

fn log(str: String) {
    crate::log::log(format!("📦assets::{}", str));
}

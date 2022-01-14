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

    match read_dir(&assets_path) {
        Ok(dir) => {
            let assets_dir: Vec<PathBuf> = dir
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
                    None => {
                        log(format!("invalid asset {:?}", entry));
                    }
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
        }
        Err(err) => {
            log("assets directory cannot be located".to_string());
        }
    }

    log(format!("scripts dir is {}", scripts_path.display()));

    match read_dir(&scripts_path) {
        Ok(dir) => {
            let scripts_dir: Vec<PathBuf> = dir
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
                    None => {
                        log(format!("invalid script {:?}", entry));
                    }
                }
            }
        }
        Err(err) => {
            log("scripts directory cannot be located".to_string());
        }
    }

    //.expect("Scripts directory failed to load")
}

pub fn get_file_name(str: String) -> String {
    let path = Path::new(&str);

    // let bits = str.split(".").collect::<Vec<_>>();
    // match bits.get(0) {
    //     Some(o) => o.to_string(),
    //     None => str,
    // }
    match path.file_stem() {
        Some(o) => match o.to_os_string().into_string() {
            Ok(n) => n,
            Err(e) => str,
        },
        None => str,
    }
}
// pub fn get_file_file_name(str: String) -> String {
//     let s = get_file_name(str);
//     let bits = s.split("/").collect::<Vec<_>>();
//     match bits.get(1) {
//         Some(o) => o.to_string(),
//         None => s,
//     }
// }

fn log(str: String) {
    crate::log::log(format!("📦assets::{}", str));
}

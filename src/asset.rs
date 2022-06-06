use std::{
    fs::{self, read_dir},
    path::{Path, PathBuf},
};

use wgpu::Device;

pub fn pack(name: &String) {
    let sources = walk_files(None);

    crate::zip_pal::pack_zip(sources, &"icon.png".to_string(), &name)
}
pub fn super_pack(name: &String) -> &str {
    // let sources = walk_files(None);
    // crate::zip_pal::pack_zip(sources, &"icon.png".to_string(), &name)
    // create::
    crate::zip_pal::pack_game_bin(name)
}

pub fn unpack(device: &Device, target: &String) {
    println!("unpack {}", target);
    let map =
        crate::zip_pal::unpack_and_walk(target, vec!["assets".to_string(), "scripts".to_string()]);

    match map.get("assets") {
        Some(dir) => {
            for item in dir {
                match Path::new(&item.0).extension() {
                    Some(e) => {
                        let file_name = &item.0;
                        if e == "glb" || e == "gltf" {
                            log(format!("loading {} as glb/gltf model", file_name));
                            crate::model::load_from_buffer(file_name, &item.1, device);
                        } else if e == "png" {
                            log(format!("loading {} as png image", file_name));
                            crate::texture::load_tex_from_buffer(&file_name.to_string(), &item.1);
                        } else {
                            log(format!("unknown file type {}", file_name));
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    //to_os_string().into_string()

    match map.get("scripts") {
        Some(dir) => {
            for item in dir {
                match Path::new(&item.0).extension() {
                    Some(e) => {
                        let file_name = &item.0;
                        let buffer = match std::str::from_utf8(&item.1.as_slice()) {
                            Ok(v) => v,
                            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
                        };
                        if e == "lua" {
                            handle_script(buffer);
                            log(format!("loading  script {}", buffer.to_string()));
                        } else {
                            log(format!("skipping file type {}", file_name));
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

pub fn init(device: &Device) {
    walk_files(Some(device));
}

fn asset_sort() {}

fn handle_script(buffer: &str) {
    let mutex = crate::lua_master.lock();
    match mutex.get() {
        Some(d) => d.load(&buffer.to_string()),
        None => log("Lua core not loaded!".to_string()),
    }
}

pub fn walk_files(
    device: Option<&Device>,
    // list: Option<HashMap<String, Vec<Vec<u8>>>>,
) -> Vec<String> {
    let assets_path = Path::new(".").join("assets");
    let scripts_path = Path::new(".").join("scripts");
    log(format!("assets dir is {}", assets_path.display()));
    let mut sources = vec![];

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
                        let file_name = &entry.into_os_string().into_string().unwrap();

                        let path = file_name.to_owned();
                        if s == "glb" || s == "gltf" {
                            if device.is_some() {
                                log(format!("loading {} as glb/gltf model", file_name));
                                crate::model::load_from_string(&path, device.unwrap());
                            }
                            sources.push(path)
                        } else if s == "png" {
                            if device.is_some() {
                                log(format!("loading {} as png image", file_name));
                                crate::texture::load_tex(&path);
                            }
                            sources.push(path)
                        } else {
                            log(format!("unknown file type {}", file_name));
                        }
                    }
                    None => {
                        log(format!("invalid asset {:?}", entry));
                    }
                }
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
                        let file_name = &entry.into_os_string().into_string().unwrap();
                        if s == "lua" {
                            log(format!("loading  script {}", file_name));

                            let input_path = Path::new("")
                                .join(file_name.to_owned())
                                .with_extension("lua");

                            // let name = crate::asset::get_file_name(input_path.to_owned());
                            let st = fs::read_to_string(input_path).unwrap_or_default();

                            // println!("script item is {}", st);

                            if device.is_some() {
                                handle_script(st.as_str())
                            }
                            sources.push(file_name.to_owned());
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
    sources
}

pub fn get_file_name(str: String) -> String {
    let path = Path::new(&str);

    match path.file_stem() {
        Some(o) => match o.to_os_string().into_string() {
            Ok(n) => n,
            Err(e) => str,
        },
        None => str,
    }
}

fn log(str: String) {
    crate::log::log(format!("📦assets::{}", str));
    println!("{}", str);
}

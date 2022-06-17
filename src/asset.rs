use std::{
    collections::{hash_map::Entry, HashMap},
    ffi::OsString,
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

    let mut sources: HashMap<String, (String, String, String, Option<&Vec<u8>>)> = HashMap::new();
    let mut configs = vec![];

    match map.get("assets") {
        Some(dir) => {
            for item in dir {
                match Path::new(&item.0).extension() {
                    Some(e) => {
                        let ext = e.to_os_string().into_string().unwrap();
                        if is_valid_type(&ext) {
                            let chonk = (item.0.clone(), ext.clone(), String::new(), Some(&item.1));
                            if ext == "ron" {
                                configs.push(chonk);
                            } else {
                                sources.insert(item.0.clone(), chonk);
                            }
                        }
                    }
                    _ => {}
                };
            }
        }
        _ => {}
    }
    parse_assets(Some(device), configs, sources);

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
// pub fn parse_files(zipped: bool, device: Option<&Device>, target: &String) -> Vec<String> {
//     let assets_path = Path::new(".").join("assets");
//     let scripts_path = Path::new(".").join("scripts");
//     let mut sources = vec![];
// }

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

pub fn is_valid_type(s: &String) -> bool {
    s == "gltf" || s == "glb" || s == "png" || s == "ron"
}

pub fn walk_files(
    device: Option<&Device>,
    // list: Option<HashMap<String, Vec<Vec<u8>>>>,
) -> Vec<String> {
    //MARK

    #[cfg(not(debug_assertions))]
    let current = match std::env::current_exe() {
        Ok(mut cur) => {
            cur.pop();
            cur
        }
        Err(er) => PathBuf::from("."),
    };

    #[cfg(debug_assertions)]
    let current = PathBuf::from(".");

    log(format!("current dir is {}", current.display()));
    let assets_path = current.join(Path::new("assets"));
    let scripts_path = current.join(Path::new("scripts"));

    log(format!("assets dir is {}", assets_path.display()));
    let mut sources: HashMap<String, (String, String, String, Option<&Vec<u8>>)> = HashMap::new();
    let mut configs = vec![];
    let mut paths = vec![];

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
                        let ext = e.to_ascii_lowercase().into_string().unwrap();

                        match entry.file_stem() {
                            Some(name) => {
                                let file_name = name.to_os_string().into_string().unwrap();
                                let path = entry.into_os_string().into_string().unwrap();
                                // sources.insert(file_name, (file_name, ext, path, None));
                                // process_asset(device, file_name, ext, buffer)
                                if is_valid_type(&ext) {
                                    let chonk =
                                        (file_name.clone(), ext.clone(), path.clone(), None);
                                    if ext == "ron" {
                                        configs.push(chonk);
                                    } else {
                                        sources.insert(file_name, chonk);
                                    }
                                    paths.push(path);
                                }
                            }
                            _ => {}
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
    parse_assets(device, configs, sources);

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
                            paths.push(file_name.to_owned());
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
    paths
}

fn parse_assets(
    device: Option<&Device>,
    configs: Vec<(String, String, String, Option<&Vec<u8>>)>,
    mut sources: HashMap<String, (String, String, String, Option<&Vec<u8>>)>,
) {
    println!("found {} template(s)", configs.len());
    for con in configs {
        // yes a double match!
        // load direct or apply zipped config buffer to string and interpret to RoN
        match match con.3 {
            Some(buffer) => {
                // TODO handle utf8 error somehow?
                crate::tile::interpret_ron(&String::from_utf8(buffer.to_vec()).unwrap())
            }
            _ => {
                println!("load the ron file");
                crate::tile::load_ron(&con.2)
            }
        } {
            Some(template) => {
                // name is either the name field or default to file name without the extension
                let name = if template.name.len() > 0 {
                    template.name.clone()
                } else {
                    con.0
                };

                println!("🟢template {}", name);
                // now locate a resource that matches the name and take ownership, if none then there's nothing to do
                match sources.entry(name) {
                    Entry::Occupied(o) => {
                        // now load the resource with config template stuff
                        let resource = o.remove_entry().1; // TODO we're removing the entry but only using it if it's a png, this may cause assets to be ignored if they're not png, buggy
                        if resource.1 == "png" {
                            // println!(
                            //     "🟢we found {} and it has a tile map with keys {} and values {}",
                            //     resource.0,
                            //     template
                            //         .tiles
                            //         .keys()
                            //         .map(|r| { format!("{:?}", r) })
                            //         .collect::<Vec<String>>()
                            //         .join(", "),
                            //     template
                            //         .tiles
                            //         .values()
                            //         .map(|r| { format!("{:?}", r) })
                            //         .collect::<Vec<String>>()
                            //         .join(", ")
                            // );

                            if resource.3.is_some() {
                                crate::texture::load_tex_from_buffer(
                                    &resource.0,
                                    &resource.3.unwrap(),
                                    Some(template),
                                );
                            } else if device.is_some() {
                                crate::texture::load_tex(&resource.2, Some(template));
                            }
                        }
                    }
                    Entry::Vacant(v) => {}
                };
            }
            _ => {}
        }
    }
    for source in sources.values() {
        let (file_name, ext, path, buffer) = source;

        if ext == "glb" || ext == "gltf" {
            log(format!("loading {} as glb/gltf model", file_name));
            match device {
                Some(d) => match buffer {
                    Some(b) => crate::model::load_from_buffer(&file_name, &b, d),
                    _ => crate::model::load_from_string(&path, d),
                },
                _ => {}
            };
        } else if ext == "png" {
            match device {
                Some(d) => {
                    log(format!("loading {} as png image", file_name));
                    match buffer {
                        Some(b) => {
                            crate::texture::load_tex_from_buffer(&file_name.to_string(), &b, None)
                        }
                        _ => crate::texture::load_tex(&path, None),
                    }
                }
                _ => {}
            };
        } else {
            log(format!(
                "unknown file {} with type {} at path {} which is {}",
                file_name,
                ext,
                path,
                "png".eq_ignore_ascii_case(ext)
            ));
        }
    }
}

// fn process_image_asset(path,file_name,buffer:Option<ec<u8>>){
//     // walk files, is device
//     log(format!("loading {} as png image", file_name));
//                                 crate::texture::load_tex(&path);
//     //unpack

//                                 log(format!("loading {} as png image", file_name));
//                             crate::texture::load_tex_from_buffer(&file_name.to_string(), &item.1);

// }

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
    println!("📦assets::{}", str);
}

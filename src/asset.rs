use std::{
    collections::{hash_map::Entry, HashMap},
    fs::{self, read_dir},
    path::{Path, PathBuf},
};

use wgpu::Device;

use crate::lua_define::LuaCore;

pub fn pack(
    name: &String,
    directory: Option<String>,
    cart_pic: Option<String>,
    lua_master: &LuaCore,
) {
    let path = determine_path(directory);
    let sources = walk_files(None, lua_master, path.clone());
    let icon = path.join(match cart_pic {
        Some(pic) => pic,
        None => "icon.png".to_string(),
    });
    crate::zip_pal::pack_zip(sources, icon, &name)
}
pub fn super_pack(name: &String) -> &str {
    // let sources = walk_files(None);
    // crate::zip_pal::pack_zip(sources, &"icon.png".to_string(), &name)
    // create::
    crate::zip_pal::pack_game_bin(name)
}

pub fn unpack(device: &Device, name: &String, file: Vec<u8>, lua_master: &LuaCore) {
    println!("unpack {}", name);
    let map =
        crate::zip_pal::unpack_and_walk(file, vec!["assets".to_string(), "scripts".to_string()]);

    let mut sources: HashMap<String, (String, String, String, Option<&Vec<u8>>)> = HashMap::new();
    let mut configs = vec![];

    match map.get("assets") {
        Some(dir) => {
            crate::lg!("unpacking {} assets", dir.len());
            for item in dir {
                let path = Path::new(&item.0);

                match path.extension() {
                    Some(e) => {
                        let ext = e.to_os_string().into_string().unwrap();
                        if is_valid_type(&ext) {
                            let name = match path.file_stem() {
                                Some(s) => s.to_os_string().into_string().unwrap(),
                                _ => item.0.clone(),
                            };
                            let chonk = (name.clone(), ext.clone(), String::new(), Some(&item.1));
                            println!("unpack🤡chonk {:?}", chonk.0);
                            if ext == "ron" || ext == "json" {
                                configs.push(chonk);
                            } else {
                                sources.insert(name, chonk);
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
            crate::lg!("unpacking {} scripts", dir.len());
            for item in dir {
                match Path::new(&item.0).extension() {
                    Some(e) => {
                        let file_name = &item.0;
                        let buffer = match std::str::from_utf8(&item.1.as_slice()) {
                            Ok(v) => v,
                            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
                        };
                        if e == "lua" {
                            handle_script(buffer, lua_master);
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

// pub fn init(device: &Device, lua_master: &LuaCore, target: &String) {
//     walk_files(Some(device), lua_master, determine_path(None));
// }

// fn asset_sort() {}

fn handle_script(buffer: &str, lua_master: &LuaCore) {
    lua_master.load(&buffer.to_string());
}

pub fn is_valid_type(s: &String) -> bool {
    s == "gltf" || s == "glb" || s == "png" || s == "ron" || s == "json"
}

pub fn check_for_auto() -> Option<String> {
    let p = PathBuf::new().join("auto.game.png");
    if p.exists() {
        Some("auto".to_string())
    } else {
        None
    }
}

pub fn determine_path(directory: Option<String>) -> PathBuf {
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
    match directory {
        Some(s) => {
            let new_dir = current.join(s);
            // if new_dir.is_dir() {
            //     new_dir
            // } else {
            //     current
            // }
            new_dir
        }
        None => current,
    }
}

pub fn make_directory(directory: String) {
    let root = determine_path(Some(directory));
    if !root.exists() {
        fs::create_dir_all(&root).unwrap();
    }
    let assets = root.join("assets").to_path_buf();
    let scripts = root.join("scripts").to_path_buf();
    if !assets.exists() {
        fs::create_dir_all(&assets).unwrap();
    }
    if !scripts.exists() {
        fs::create_dir_all(&scripts).unwrap();
    }
    // scripts.join("main.lua").write_to_file("main.lua").unwrap();

    fs::write(
        scripts.join("main.lua"),
        "
example = spawn('example', 12, rnd() * 3. - 1.5, rnd() * 3. - 1.5)
bg(1, 1, .4, 1)
function main()
    log('main runs once everything has loaded')
end
function loop()
    example.x = example.x + rnd() * 0.1 - 0.05
    example.y = example.y + rnd() * 0.1 - 0.05
end",
    )
    .unwrap();

    crate::texture::simple_square(16, assets.join("example.png"));
}

pub fn walk_files(
    device: Option<&Device>,
    // list: Option<HashMap<String, Vec<Vec<u8>>>>,
    lua_master: &LuaCore,
    current_path: PathBuf,
) -> Vec<String> {
    //MARK

    log(format!("current dir is {}", current_path.display()));
    let assets_path = current_path.join(Path::new("assets"));
    let scripts_path = current_path.join(Path::new("scripts"));

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
                                    // println!("load🤡chonk {:?}", chonk.0);
                                    if ext == "ron" || ext == "json" {
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
        _ => {
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
                                handle_script(st.as_str(), lua_master)
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
        _ => {
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
                if con.1 == "ron" {
                    crate::template::interpret_ron(&String::from_utf8(buffer.to_vec()).unwrap())
                } else if con.1 == "json" {
                    crate::template::interpret_json(
                        &con.0,
                        &String::from_utf8(buffer.to_vec()).unwrap(),
                    )
                } else {
                    None
                }
            }
            None => {
                if con.1 == "ron" {
                    crate::template::load_ron(&con.2)
                } else if con.1 == "json" {
                    crate::template::load_json(&con.0, &con.2)
                } else {
                    None
                }
            }
        } {
            Some(template) => {
                // name is either the name field or default to file name without the extension
                log(format!("loaded template {} or {}", con.0, template.name));
                let name = if template.name.len() > 0 {
                    template.name.clone()
                } else {
                    con.0
                };

                // println!("🟢template {}", name);
                // now locate a resource that matches the name and take ownership, if none then there's nothing to do
                // println!("checking {} sources", sources.len());
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
                                    Some(&template),
                                );
                            } else if device.is_some() {
                                crate::texture::load_tex(&resource.2, Some(&template));
                            }

                            if template.anims.len() > 0 {
                                // println!(
                                //     "🟣we found {} and it has {} animations",
                                //     resource.0,
                                //     template.anims.len()
                                // );
                                for a in template.anims {
                                    // println!("🟢{:?}", a.1);
                                    let v =
                                        a.1.iter()
                                            .map(|t| crate::texture::get_tex(&t))
                                            .collect::<Vec<glam::Vec4>>();
                                    crate::texture::set_anims(&a.0, v, 8);
                                }
                            }
                        }
                    }
                    Entry::Vacant(_) => {}
                };
            }
            None => {}
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
                Some(_) => {
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

// pub fn get_file_name(str: String) -> String {
//     let path = Path::new(&str);

//     match path.file_stem() {
//         Some(o) => match o.to_os_string().into_string() {
//             Ok(n) => n,
//             Err(_) => str,
//         },
//         None => str,
//     }
// }

fn log(str: String) {
    crate::log::log(format!("📦assets::{}", str));
    println!("📦assets::{}", str);
}

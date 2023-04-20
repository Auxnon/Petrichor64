use std::{
    collections::HashMap,
    fs::{self, read_dir},
    path::{Path, PathBuf},
};

use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};
use imageproc::drawing::draw_filled_rect_mut;
use regex::Regex;
#[cfg(feature = "headed")]
use wgpu::Device;

use crate::model::ModelManager;
#[cfg(feature = "headed")]
use crate::texture::TexManager;
use crate::{
    global::Global,
    log::{LogType, Loggy},
    lua_define::{LuaCore, LuaResponse},
    world::World,
};

// 1.0.0 Applesauce
// 2.1.0 Avocado
pub fn get_codex_version_string() -> String {
    "-- Codex 3.0.0 \"Artichoke\"".to_owned()
}

pub fn pack(
    #[cfg(feature = "headed")] tex_manager: &mut TexManager,
    model_manager: &mut ModelManager,
    world: &mut World,
    bundle_id: u8,
    lua_master: &LuaCore,
    // name: &String,
    // directory: Option<String>,
    // cart_pic: Option<String>,
    com_hash: HashMap<String, String>,
    regular_com: Vec<String>,
    current_game_dir: Option<String>,
    loggy: &mut Loggy,
    debug: bool,
) {
    let dir = match com_hash.get("i") {
        Some(path) => Some(path.to_owned()),
        None => {
            if regular_com.len() > 0 {
                Some(regular_com[0].to_owned())
            } else {
                match current_game_dir {
                    Some(dir) => Some(dir),
                    None => None,
                }
            }
        }
    };
    let path = determine_path(dir);

    let name = match com_hash.get("n") {
        Some(name) => name,
        None => {
            let p = path.file_stem();
            // println!("p: {:?}", p);
            match p {
                Some(pp) => pp.to_str().unwrap_or("unknown"),
                None => "unknown",
            }
        }
    };
    println!("name: {}", name);
    let pack_name = if name.contains(".") {
        name.to_owned()
    } else {
        format!("{}.game.png", name)
    };
    println!("pack name: {}", pack_name);

    let sources = walk_files(
        #[cfg(feature = "headed")]
        None,
        #[cfg(feature = "headed")]
        tex_manager,
        model_manager,
        false,
        world,
        bundle_id,
        lua_master,
        path.clone(),
        loggy,
        debug,
    );

    let icon = path.join(match com_hash.get("c") {
        Some(pic) => pic.to_owned(),
        None => "icon.png".to_string(),
    });
    crate::zip_pal::pack_zip(sources, icon, &pack_name, loggy)
}
pub fn super_pack(name: &str) -> &str {
    // let sources = walk_files(None);
    // crate::zip_pal::pack_zip(sources, &"icon.png".to_string(), &name)
    // create::
    crate::zip_pal::pack_game_bin(name)
}

pub fn unpack(
    #[cfg(feature = "headed")] device: &Device,
    #[cfg(feature = "headed")] tex_manager: &mut TexManager,
    model_manager: &mut ModelManager,
    world: &mut World,
    bundle_id: u8,
    lua_master: &LuaCore,
    name: &str,
    file: Vec<u8>,
    loggy: &mut Loggy,
    debug: bool,
) {
    if debug {
        loggy.log(LogType::Config, &format!("unpack {}", name));
    }
    let map = crate::zip_pal::unpack_and_walk(
        file,
        vec!["assets".to_string(), "scripts".to_string()],
        loggy,
    );

    let mut sources: HashMap<String, (String, String, String, Option<&Vec<u8>>)> = HashMap::new();
    #[cfg(feature = "headed")]
    let mut configs = vec![];

    #[cfg(feature = "headed")]
    match map.get("assets") {
        Some(dir) => {
            if debug {
                loggy.log(LogType::Config, &format!("unpacking {} assets", dir.len()));
            }
            for (item_name, item_buffer) in dir {
                let path = Path::new(&item_name);

                match path.extension() {
                    Some(e) => {
                        let ext = e.to_os_string().into_string().unwrap();
                        if is_valid_type(&ext) {
                            let name = match path.file_stem() {
                                Some(s) => s.to_os_string().into_string().unwrap(),
                                _ => item_name.clone(),
                            };
                            let chonk =
                                (name.clone(), ext.clone(), String::new(), Some(item_buffer));
                            // println!("unpack🤡chonk {:?}", chonk.0);
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
    #[cfg(feature = "headed")]
    parse_assets(
        tex_manager,
        model_manager,
        world,
        bundle_id,
        Some(device),
        configs,
        sources,
        loggy,
        debug,
    );

    match map.get("scripts") {
        Some(dir) => {
            loggy.log(LogType::Config, &format!("unpacking {} scripts", dir.len()));
            for (item_name, item_buffer) in dir {
                match Path::new(&item_name).extension() {
                    Some(e) => {
                        let file_name = &item_name;
                        let buffer = match std::str::from_utf8(&item_buffer.as_slice()) {
                            Ok(v) => v,
                            Err(e) => panic!("Invalid UTF-8 sequence: {}", e),
                        };
                        if e == "lua" {
                            if debug {
                                loggy.log(
                                    LogType::LuaSys,
                                    &format!("loading script {}", file_name), //buffer.to_string()),
                                );
                            }
                            handle_script(buffer, lua_master);
                        } else {
                            if debug {
                                loggy.log(
                                    LogType::Config,
                                    &format!("skipping file type {}", file_name),
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn handle_script(buffer: &str, lua_master: &LuaCore) -> [u16; 3] {
    let mut ver = [0, 0, 0];
    let l = buffer.lines();
    let reg = Regex::new(r"[0-9]+").unwrap();
    l.take(5).for_each(|line| {
        if line.starts_with("--") && line.to_lowercase().contains("codex") {
            let v = line.split(".").collect::<Vec<&str>>();
            ver[0] = to_num(&v, 0, &reg);
            ver[1] = to_num(&v, 1, &reg);
            ver[2] = to_num(&v, 2, &reg);
        }
    });
    lua_master.async_load(&buffer.to_string());
    ver
}

fn to_num(s: &Vec<&str>, n: usize, reg: &Regex) -> u16 {
    match s.get(n) {
        Some(m) => match reg.find(m) {
            Some(mm) => {
                let ss = mm.as_str();
                ss.parse::<u16>().unwrap_or(0)
            }
            None => 0,
        },
        None => 0,
    }
}

pub fn show_config() {
    let p = determine_path(Some(".petrichor64/config.lua".to_owned()));
    if !p.exists() {
        println!("config not found, creating one");
        make_config();
    }
    open_dir_from_path(&p)
}

pub fn make_config() {
    let p = determine_path(Some(".petrichor64".to_owned()));
    if !p.exists() {
        fs::create_dir_all(p).unwrap();
    }
    let p = determine_path(Some(".petrichor64/config.lua".to_owned()));
    if !p.exists() {
        fs::write(
            p,
            r#"
-- dev = true
-- alias = {
--   a = "load apps/tillys-silly-uwu-quest",
--   crt = "attr{modernize=0}",
-- }"#,
        )
        .unwrap();
    }
}

pub fn check_config() -> bool {
    let p = determine_path(Some(".petrichor64/config.lua".to_owned()));
    p.exists()
}
pub fn parse_config(globals: &mut Global, lua: &LuaCore, loggy: &mut Loggy) -> Option<String> {
    let p = determine_path(Some(".petrichor64/config.lua".to_owned()));
    let mut result = None;
    if p.exists() {
        if let Ok(buffer) = std::fs::read_to_string(p) {
            lua.load(&buffer.to_string());

            globals.debug = eval_bool(lua.func("dev"));
            if let LuaResponse::Table(t) = lua.func("alias") {
                if globals.debug {
                    loggy.log(LogType::Config, &format!("{} aliases", t.len()));
                }
                // crate::lg!("{:?} aliases", t);
                for (k, v) in t {
                    if globals.debug {
                        loggy.log(LogType::Config, &format!("{} -> {}", k, v));
                    }
                    globals.aliases.insert(k, v);
                }
            }
            if let LuaResponse::String(s) = lua.func("init") {
                result = Some(s);
            }
            if globals.debug {
                loggy.log(LogType::Config, &"dev is enabled");
            }
        };
    }
    return result;
}

fn eval_bool(res: LuaResponse) -> bool {
    match res {
        LuaResponse::String(s) => s == "true",
        LuaResponse::Number(n) => n == 1.0,
        LuaResponse::Integer(i) => i != 0,
        LuaResponse::Boolean(b) => b,
        _ => false,
    }
}

pub fn is_valid_type(s: &String) -> bool {
    s == "gltf" || s == "glb" || s == "png" || s == "ron" || s == "json"
}

pub fn check_for_auto() -> Option<String> {
    let mut p;
    #[cfg(target_os = "macos")]
    {
        // if it's built it's 2 levels up, if it's bundled it's 4 levels up
        let pp = std::env::current_exe()
            .unwrap()
            .join("..")
            .join("..")
            .join("..");
        let p1 = pp.join("..");
        let absolute = p1.canonicalize().unwrap();
        // println!("checking for auto {:?}", absolute);
        p = absolute.join("auto.game.png");

        // also check one level down into the app bundle
        if !p.exists() {
            let absolute = pp.canonicalize().unwrap();
            p = absolute.join("auto.game.png");
        }
    }

    #[cfg(not(target_os = "macos"))]
    match std::env::current_exe() {
        Ok(pp) => {
            p = pp.join("auto.game.png");
        }
        Err(_) => {
            return None;
        }
    }
    // p = std::env::current_exe().unwrap().join("auto.game.png");
    // let p = dirs::.unwrap().join("auto.game.png");
    // for a in p.read_dir().unwrap() {
    //     println!("checking for auto {:?}", a);
    // }

    // let p = PathBuf::new().join("..").join("auto.game.png");
    // println!("checking for auto {:?}", p);

    // #[cfg(not(target_os = "macos"))]
    // let p = PathBuf::new().join("auto.game.png");

    if p.exists() {
        Some(p.to_str().unwrap().to_owned())
    } else {
        None
    }
}

pub fn determine_path(directory: Option<String>) -> PathBuf {
    #[cfg(not(debug_assertions))]
    let current = match dirs::home_dir() {
        Some(cur) => cur,
        None => PathBuf::from(".").to_path_buf(),
    };

    #[cfg(debug_assertions)]
    let current = PathBuf::from(".");

    match directory {
        Some(s) => {
            let new_dir = current.join(s);

            new_dir
        }
        None => current,
    }
}

pub fn make_directory(
    directory: &str,
    command_map: &HashMap<String, (String, String)>,
    loggy: &mut Loggy,
) {
    let root = determine_path(Some(directory.to_string()));
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
        get_codex_version_string()
            + "
example = make('example', rnd() * 3. - 1.5,12, rnd() * 3. - 1.5)
sky:fill('FF5')
function main()
    cout 'main runs once everything has loaded'
end
function loop()
    example.x = example.x + rnd() * 0.1 - 0.05
    example.z = example.z + rnd() * 0.1 - 0.05
end",
    )
    .unwrap();

    fs::write(scripts.join("ignore.lua"), make_codex_file(command_map));

    simple_square(16, assets.join("example.png"), loggy);
    simple_square(16, root.join("icon.png"), loggy);
}

pub fn walk_files(
    #[cfg(feature = "headed")] device: Option<&Device>,
    #[cfg(feature = "headed")] tex_manager: &mut TexManager,
    model_manager: &mut ModelManager,
    activate: bool,
    world: &mut World,
    bundle_id: u8,
    lua_master: &LuaCore,
    current_path: PathBuf,
    loggy: &mut Loggy,
    debug: bool,
) -> Vec<String> {
    loggy.log(
        LogType::Config,
        &format!("current dir is {}", current_path.display()),
    );
    let assets_path = current_path.join(Path::new("assets"));
    let scripts_path = current_path.join(Path::new("scripts"));

    loggy.log(
        LogType::Config,
        &format!("assets dir is {}", assets_path.display()),
    );
    let mut sources: HashMap<String, (String, String, String, Option<&Vec<u8>>)> = HashMap::new();
    let mut configs = vec![];
    let mut paths = vec![];
    let mut version = [0; 3];

    match read_dir(&assets_path) {
        Ok(dir) => {
            let assets_dir: Vec<PathBuf> = dir
                .filter(Result::is_ok)
                .map(|e| e.unwrap().path())
                .collect();

            for entry in assets_dir {
                loggy.log(
                    LogType::Config,
                    &format!("asset to load {}", entry.display()),
                );
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
                        loggy.log(LogType::ConfigError, &format!("invalid asset {:?}", entry));
                    }
                }
            }
        }
        _ => {
            loggy.log(
                LogType::ConfigError,
                &"assets directory cannot be located".to_string(),
            );
        }
    }
    #[cfg(feature = "headed")]
    parse_assets(
        tex_manager,
        model_manager,
        world,
        bundle_id,
        device,
        configs,
        sources,
        loggy,
        debug,
    );

    loggy.log(
        LogType::Config,
        &format!("scripts dir is {}", scripts_path.display()),
    );
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
                            if file_name.ends_with("ignore.lua") {
                                loggy.log(LogType::Config, "skipping ignore script");
                            } else {
                                loggy
                                    .log(LogType::Config, &format!("reading script {}", file_name));

                                let input_path = Path::new("")
                                    .join(file_name.to_owned())
                                    .with_extension("lua");

                                // let name = crate::asset::get_file_name(input_path.to_owned());
                                let st = fs::read_to_string(input_path).unwrap_or_default();

                                // println!("script item is {}", st);

                                if activate {
                                    let ver = handle_script(st.as_str(), lua_master);
                                    if ver[0] > version[0]
                                        || ver[1] > version[1]
                                        || ver[2] > version[2]
                                    {
                                        version = ver;
                                    }
                                }
                                paths.push(file_name.to_owned());
                            }
                        } else {
                            loggy.log(
                                LogType::ConfigError,
                                &format!("skipping file type {}", file_name),
                            );
                        }
                    }
                    None => {
                        loggy.log(LogType::ConfigError, &format!("invalid script {:?}", entry));
                    }
                }
            }
        }
        _ => {
            loggy.log(LogType::ConfigError, &"scripts directory cannot be located");
        }
    }

    loggy.log(LogType::Config, &format!("app version is {:?}", version));
    //.expect("Scripts directory failed to load")
    paths
}

#[cfg(feature = "headed")]
fn parse_assets(
    tex_manager: &mut TexManager,
    model_manager: &mut ModelManager,
    world: &mut World,
    bundle_id: u8,
    device: Option<&Device>,
    configs: Vec<(String, String, String, Option<&Vec<u8>>)>,
    mut sources: HashMap<String, (String, String, String, Option<&Vec<u8>>)>,
    loggy: &mut Loggy,
    debug: bool,
) {
    use std::collections::hash_map::Entry;

    loggy.log(
        LogType::Config,
        &format!("found {} template(s)", configs.len()),
    );
    for con in configs {
        // yes a double match!
        // load direct or apply zipped config buffer to string and interpret to RoN
        match match con.3 {
            Some(buffer) => {
                // TODO handle utf8 error somehow?
                if con.1 == "ron" {
                    crate::template::interpret_ron(
                        &String::from_utf8(buffer.to_vec()).unwrap(),
                        loggy,
                    )
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
                    crate::template::load_ron(&con.2, loggy)
                } else if con.1 == "json" {
                    crate::template::load_json(&con.0, &con.2)
                } else {
                    None
                }
            }
        } {
            Some(template) => {
                // name is either the name field or default to file name without the extension
                loggy.log(
                    LogType::Config,
                    &format!("loaded template {} or {}", con.0, template.name),
                );
                let name = if template.name.len() > 0 {
                    template.name.clone()
                } else {
                    con.0
                };

                // println!("🟢template {}", name);
                // now locate a resource that matches the name and take ownership, if none then there's nothing to do
                // println!("checking {} sources", sources.len());
                match sources.entry(name.clone()) {
                    Entry::Occupied(o) => {
                        // now load the resource with config template stuff
                        let resource = o.remove_entry().1; // TODO we're removing the entry but only using it if it's a png, this may cause assets to be ignored if they're not png, buggy
                        if resource.1 == "png" {
                            if resource.3.is_some() {
                                tex_manager.load_tex_from_buffer(
                                    world,
                                    &resource.0,
                                    &resource.3.unwrap(),
                                    bundle_id,
                                    Some(&template),
                                    loggy,
                                );
                            } else if device.is_some() {
                                tex_manager.load_tex(
                                    world,
                                    &resource.2,
                                    bundle_id,
                                    Some(&template),
                                    loggy,
                                );
                            }

                            if template.anims.len() > 0 {
                                // println!(
                                //     "🟣we found {} and it has {} animations",
                                //     resource.0,
                                //     template.anims.len()
                                // );
                                for a in template.anims {
                                    let compound = format!("{}.{}", &name, a.0).to_lowercase();
                                    // println!("🟢{:?}", a.1);
                                    let v =
                                        a.1.iter()
                                            .map(|t| tex_manager.get_tex(&t))
                                            .collect::<Vec<glam::Vec4>>();
                                    if v.len() > 0 {
                                        tex_manager.set_anims(&compound, v, 8, a.2);
                                    } else {
                                        loggy.log(
                                            LogType::ConfigError,
                                            &format!(
                                                "0 frames in animation {}, disposing",
                                                compound
                                            ),
                                        );
                                    }
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
            loggy.log(
                LogType::Model,
                &format!("loading {} as glb/gltf model", file_name),
            );
            match device {
                Some(d) => match buffer {
                    Some(b) => model_manager.load_from_buffer(
                        d,
                        tex_manager,
                        world,
                        bundle_id,
                        &file_name,
                        &b,
                        loggy,
                        debug,
                    ),
                    _ => model_manager.load_from_string(
                        d,
                        tex_manager,
                        world,
                        bundle_id,
                        &path,
                        loggy,
                        debug,
                    ),
                },
                _ => {}
            };
        } else if ext == "png" {
            match device {
                Some(_) => {
                    loggy.log(
                        LogType::Texture,
                        &format!("loading {} as png image", file_name),
                    );
                    match buffer {
                        Some(b) => tex_manager.load_tex_from_buffer(
                            world,
                            &file_name.to_string(),
                            &b,
                            bundle_id,
                            None,
                            loggy,
                        ),
                        _ => tex_manager.load_tex(world, &path, bundle_id, None, loggy),
                    }
                }
                _ => {}
            };
        } else {
            loggy.log(
                LogType::ConfigError,
                &format!(
                    "unknown file {} with type {} at path {} which is {}",
                    file_name,
                    ext,
                    path,
                    "png".eq_ignore_ascii_case(ext)
                ),
            );
        }
    }
}

pub fn open_dir(s: &str) {
    let p = determine_path(Some(s.to_string()));
    let p2 = if p.exists() { p } else { determine_path(None) };
    open_dir_from_path(&p2);
}
pub fn open_dir_from_path(p: &Path) {
    use std::process::Command;
    Command::new("open").arg(p).spawn();
}

pub fn get_logo() -> Vec<u8> {
    #[cfg(not(windows))]
    macro_rules! sp {
        () => {
            "/"
        };
    }

    #[cfg(windows)]
    macro_rules! sp {
        () => {
            r#"\"#
        };
    }
    include_bytes!(concat!("..", sp!(), "logo.game.png")).to_vec()
}
pub fn get_b() -> Vec<u8> {
    #[cfg(not(windows))]
    macro_rules! sp {
        () => {
            "/"
        };
    }

    // TODO when cross compiling for windows you'll need this to be the other slash... should be a different build process
    #[cfg(windows)]
    macro_rules! sp {
        () => {
            r#"\"#
        };
    }
    include_bytes!(concat!("..", sp!(), "b.game.png")).to_vec()
}
pub fn load_img(str: &str, loggy: &mut Loggy) -> Result<DynamicImage, image::ImageError> {
    let text = Path::new("assets").join(str).to_str().unwrap().to_string();
    //Path::new(".").join("entities");
    load_img_nopath(&text, loggy)
}

pub fn load_img_nopath(str: &str, loggy: &mut Loggy) -> Result<DynamicImage, image::ImageError> {
    loggy.log(LogType::Texture, &format!("loading image {}", str));

    let img = image::open(str);

    // The dimensions method returns the images width and height.
    //println!("dimensions height {:?}", img.height());

    img
}

pub fn load_img_from_buffer(buffer: &[u8]) -> Result<DynamicImage, image::ImageError> {
    let img = image::load_from_memory(buffer);
    img
}

pub fn simple_square(size: u32, path: PathBuf, loggy: &mut Loggy) {
    let mut img: RgbaImage = ImageBuffer::new(size, size);
    let magenta = Rgba([255u8, 0u8, 255u8, 255u8]);
    draw_filled_rect_mut(
        &mut img,
        imageproc::rect::Rect::at(0, 0).of_size(size, size),
        magenta,
    );
    match image::save_buffer_with_format(
        path,
        &img,
        size,
        size,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    ) {
        Err(err) => loggy.log(
            LogType::TextureError,
            &format!("could not save example image: {}", err),
        ),
        _ => {}
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

// fn log(str: String) {
//     crate::log::log(format!("📦assets::{}", str));
//     println!("📦assets::{}", str);
// }
pub fn make_codex_file(command_map: &HashMap<String, (String, String)>) -> String {
    let mut s = get_codex_version_string()
        + "
---@diagnostic disable: duplicate-doc-field, missing-return
---@meta

---@class mouse
---@field x number
---@field y number
---@field dx number delta x
---@field dy number delta y
---@field m1 boolean mouse 1
---@field m2 boolean mouse 2
---@field m3 boolean mouse 3
-- @field scroll number scroll delta
---@field vx number unprojection x
---@field vy number unprojection y
---@field vz number unprojection z
---@return mouse

---@class attributes
---@field resolution number artificial resolution
---@field lock boolean
---@field fog number 0 is off
---@field fullscreen boolean
---@field mouse_grab boolean
---@field size integer[] width, height of window
---@field title string
---@field modernize boolean must be false or 0 for the remainder to work
---@field dark number
---@field glitch number[]
---@field curvature number
---@field flatness number
---@field high number
---@field low number
---@field bleed number

---@class cam_params
---@field pos number[]? x, y, z
---@field rot number[]? azimuth, altitude

--- @class model_data
--- @field t string[]? texture assets
--- @field q number[][]? quads
--- @field v number[][]? vertices
--- @field u number[][]? uvs
--- @field i integer[][]? indicies

--- @class entity
--- @field x number x position
--- @field y number y position
--- @field z number z position
--- @field rx number rotation x
--- @field ry number rotation y
--- @field rz number rotation z
--- @field vx number velocity x
--- @field vy number velocity y
--- @field vz number velocity z
--- @field flipped number texture flip x axis
--- @field scale number uniform scale factor 1 is 100%
--- @field id integer assigned by engine, killable
--- @field tex string texture asset
--- @field asset string model or blocked texture asset
--- @field anim fun(self:entity,animation:string,force?:boolean) change animation, force marks change even if already playing
--- @field kill fun(self:entity) destroy entity

--- @alias gunit number | integer | string
--- @alias rgb number[] | integer[] | string

--- @class image
--- @field line fun(self:image, x:gunit, y:gunit, x2:gunit, y2:gunit, rgb?:rgb) draw line on image
--- @field rect fun(self:image, x:gunit, y:gunit, w:gunit, h:gunit, rgb?:rgb) draw rectangle on image
--- @field rrect fun(self:image ,x:gunit, y:gunit, w:gunit, h:gunit,ro:gunit, rgb?:rgb) draw rounded rectangle on image
--- @field text fun(self:image, txt:string, x?:gunit, y?:gunit, rgb?:rgb) draw text on image
--- @field img fun(self:image, im:image, x?:gunit, y?:gunit) draw another image on image
--- @field pixel fun(self:image, x:integer, y:integer,rgb?:rgb) draw pixel directly on image
--- @field clr fun(self:image) clear image
--- @field fill fun(self:image, rgb?:rgb) fill image with color
--- @field raw fun(self:image):integer[] image return raw pixel data
--- @field copy fun(self:image):image clones to new image

--- @class connection
--- @field send fun(self:connection, data:string) send data to connection
--- @field recv fun(self:connection):string | nil receive data from connection
--- @field test fun(self:connection):string | nil test if connection is still alive, returns string for error, 'safe close' for no error
--- @field kill fun(self:connection) close connection


--- @type number ~3.1457
pi = nil
--- @type number ~6.2914
tau = nil
--- @type image image raster for the front screen
gui = nil
--- @type image image raster for the back screen or 'sky'
sky = nil

--- shorthand for gui:text
function text(...) end

--- shorthand for gui:line
function line(...) end

--- shorthand for gui:rect
function rect(...) end

--- shorthand for gui:rrect
function rrect(...) end

--- shorthand for gui:img
function img(...) end

--- shorthand for gui:pixel
function pixel(...) end

--- shorthand for gui:fill
function fill(...) end

--- shorthand for gui:clr
function clr(...) end



";
    for (name, (desc, examp)) in command_map {
        s += &format!("-- {}\n{}\n\n\n", desc.trim(), examp.trim());
    }
    s
}

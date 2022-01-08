use std::{
    fs::read_dir,
    path::{Path, PathBuf},
};

use wgpu::Device;

pub fn init(device: &Device) {
    let input_path = Path::new(".").join("assets");
    log(format!("dir is {}", input_path.display()));

    crate::model::load("package", device);

    let dir: Vec<PathBuf> = read_dir(&input_path)
        .expect("Assets directory failed to load")
        .filter(Result::is_ok)
        .map(|e| e.unwrap().path())
        .collect();

    for entry in dir {
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
}

fn log(str: String) {
    crate::log::log(format!("📦assets::{}", str));
}

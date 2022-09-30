use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::{
    ent::EntityUniforms,
    model::{Instance, InstanceRaw},
};
use glam::vec3;
use mlua::{UserData, UserDataMethods};
use wgpu::util::DeviceExt;

use crate::{ent::Ent, lua_ent::LuaEnt};
// use serde::Deserialize;
// use std::{
//     collections::HashMap,
//     fs::{read_dir, File},
//     path::PathBuf,
//     sync::Arc,
// };

pub struct EntManager {
    // pub ent_table: Mutex<mlua::Table<'static>>,
    pub entities: HashMap<i64, Ent>,
    // pub create: Vec<LuaEnt>,
    pub ent_table: Vec<Arc<Mutex<LuaEnt>>>,
    pub uniform_alignment: u32,
    pub instances: Vec<Instance>,
    pub instance_buffer: wgpu::Buffer,
}
impl EntManager {
    pub fn new(device: &wgpu::Device) -> EntManager {
        EntManager {
            // ent_table: Mutex::new(),
            ent_table: vec![],
            entities: HashMap::new(),
            instances: vec![],
            instance_buffer: EntManager::build_buffer(&vec![], device),
            uniform_alignment: 0,
        }
    }

    pub fn build_buffer(instances: &Vec<Instance>, device: &wgpu::Device) -> wgpu::Buffer {
        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    pub fn rebuild_instance_buffer(&mut self, device: &wgpu::Device) {
        self.instance_buffer = EntManager::build_buffer(&self.instances, device);
    }

    pub fn build_instance_buffer(
        instance_data: &Vec<EntityUniforms>,
        device: &wgpu::Device,
    ) -> wgpu::Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    // pub fn add(&mut self, x: f32, y: f32, z: f32) -> LuaEnt {
    //     let mut ent = LuaEnt::empty();
    //     ent.x = x;
    //     ent.y = y;
    //     ent.z = z;
    //     // self.create.push(ent.clone());
    //     ent
    // }
    pub fn check_create(&mut self) {
        // if self.create.len() > 0 {
        //     println!("create an ent");
        //     let typeOf = true;
        //     for c in &self.create {
        //         self.entities.push(Ent::new(
        //             vec3(c.x, c.y, c.z),
        //             0.,
        //             if typeOf { 1. } else { 1. },
        //             0.,
        //             if typeOf {
        //                 "chicken".to_string()
        //             } else {
        //                 "package".to_string()
        //             },
        //             if typeOf {
        //                 "plane".to_string()
        //             } else {
        //                 "package".to_string()
        //             },
        //             (self.entities.len() as u32 * self.uniform_alignment) as u32,
        //             typeOf,
        //             None, //Some("walker".to_string()),
        //         ));
        //     }
        //     self.create.clear();
        // }
    }

    pub fn get_uuid() -> String {
        uuid::Uuid::new_v4().to_simple().to_string()
    }
    // pub fn get_ents(self){
    //     self.entities.update()
    // }

    pub fn create_from_lua(&mut self, wrapped_lua: Arc<Mutex<LuaEnt>>) {
        let lua = wrapped_lua.lock().unwrap();
        // let lua=guard.
        let id = lua.get_id();
        let mut asset = lua.get_asset();
        if asset.len() == 0 {
            asset = "example".to_string();
        }

        // MARK should change plane to a model if the texture doesn't exist as one
        let ent = Ent::new_dynamic(
            vec3(lua.x as f32, lua.y as f32, lua.z as f32),
            0.,
            1.,
            0.,
            asset,
            self.uniform_alignment * (id + 1) as u32,
        );

        self.entities.insert(lua.get_id(), ent);
        drop(lua);
        self.ent_table.push(wrapped_lua);
    }

    pub fn get_from_lua(&self, lua: &LuaEnt) -> Option<&Ent> {
        let id = lua.get_id();

        self.entities.get(&id)
    }
    pub fn get_from_id(&self, id: i64) -> Option<&Ent> {
        self.entities.get(&id)
    }
    pub fn get_from_id_mut(&mut self, id: i64) -> Option<&mut Ent> {
        self.entities.get_mut(&id)
    }
    pub fn swap_tex(&mut self, tex: &String, ent_id: i64) {
        match self.entities.get_mut(&ent_id) {
            Some(e) => {
                e.tex = crate::texture::get_tex(tex);
            }
            _ => {}
        }
    }

    pub fn destroy_from_lua(&mut self, lua: &LuaEnt) {
        self.entities.remove(&lua.get_id());
    }
    pub fn reset(&mut self) {
        self.entities.clear();
        self.ent_table.clear();
        // self.uniform_alignment = 0;
    }

    pub fn check_ents(&mut self) {
        let mut v: Vec<LuaEnt> = vec![];
        for lua_ent in self.ent_table.iter_mut() {
            match lua_ent.try_lock() {
                Ok(mut l) => {
                    if l.is_dirty() {
                        l.clear_dirt();
                        // println!("pre dirty in array {}", l.get_tex());
                        v.push(l.clone());
                    } else {
                        // println!("not dirty");
                    }
                }
                _ => {}
            }
        }
        if v.len() > 0 {
            for l in v {
                // println!("we have in array ");
                match self.get_from_id_mut(l.get_id()) {
                    Some(e) => {
                        if l.is_anim() {
                            // println!("anim {}", l.get_tex());
                            match crate::texture::ANIMATIONS.read().get(l.get_tex()) {
                                Some(t) => {
                                    // println!("we found {} with {:?}", l.get_tex(), t);
                                    e.anim = t.0.clone();
                                    e.anim_speed = t.1;
                                }
                                _ => {}
                            }
                        } else {
                            e.tex = crate::texture::get_tex(l.get_tex());
                            if e.anim.len() > 0 {
                                e.anim = vec![];
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
// use crate::model::Model;

// #[derive(Default, Debug, Deserialize)]
// pub struct PreEntSchema {
//     name: String,
//     resource: String,
//     #[serde(default)]
//     anims: HashMap<String, (u16, u16)>,
//     #[serde(default)]
//     resource_size: Vec<u16>,
//     logic: String,
// }

// pub struct EntSchema {
//     pub name: String,
//     pub resource: String,
//     pub albedo: cgmath::Vector4<f32>,
//     pub normals: cgmath::Vector4<f32>,
//     pub model: Arc<Model>,

//     //pub anims: HashMap<String, (u16, u16)>,
//     //pub resource_size: Vec<u16>,
//     pub brain: String,
//     pub effects: cgmath::Vector4<u32>,
// }
// impl EntSchema {
//     // pub fn get_anim(&self, name: String) -> (u16, u16) {
//     //     match self.anims.get(&name) {
//     //         Some(&o) => o,
//     //         None => (0, 0),
//     //     }
//     // }
// }

// lazy_static! {
//     pub static ref ent_map: Arc<HashMap<String, EntSchema>> = Arc::new(HashMap::new());
//     pub static ref default_ent_schema: Arc<OnceCell<EntSchema>> = Arc::new(OnceCell::new());
// }

// pub fn init() {
//     // default_ent_schema.get_or_init(||)
//     let input_path = Path::new(".").join("entities");
//     //let input_path = format!("{}/entities/", env!("CARGO_MANIFEST_DIR"));
//     log(format!("ent dir is {}", input_path.display()));
//     let dir: Vec<PathBuf> = read_dir(&input_path)
//         .expect("Entity directory failed to load")
//         .filter(Result::is_ok)
//         .map(|e| e.unwrap().path())
//         .collect();

//     for entry in dir {
//         println!("entity to load {}", entry.display());
//         let f = File::open(&entry).expect("Failed opening an entity file");
//         let schema: PreEntSchema = match from_reader(f) {
//             Ok(x) => x,
//             Err(e) => {
//                 println!("Failed to apply entity RON schema, defaulting: {}", e);
//                 //std::process::exit(1);
//                 PreEntSchema::default()
//             }
//         };
//         let mut ent;

//         if (schema.resource_size.len() > 2) {
//             //then it's a 3d resource!
//             let text = format!("assets/{}.glb", schema.resource);
//             let mesh = three_loader::load(&text);

//             ent = EntSchema {
//                 name: schema.name,
//                 anims: schema.anims,
//                 resource: schema.resource,
//                 albedo: Texture2D::empty(),
//                 normals: Texture2D::empty(),
//                 mesh,
//                 logic: schema.logic,
//                 resource_size: schema.resource_size,
//                 flat: false,
//             };
//         } else {
//             let text = format!("assets/{}.png", schema.resource);
//             let ntext = format!("assets/{}_n.png", schema.resource);
//             //println!("loaded texture {}", text);
//             let albedo = load_texture(&text[..]).await.unwrap_or(Texture2D::empty());
//             //println!(" texture width {}", albedo.width());
//             let normals = load_texture(&ntext[..]).await.unwrap_or(Texture2D::empty());
//             let mesh = vec![Mesh {
//                 vertices: [].to_vec(),
//                 indices: [].to_vec(),
//                 texture: Some(Texture2D::empty()),
//             }];
//             normals.set_filter(FilterMode::Nearest);
//             albedo.set_filter(FilterMode::Nearest);
//             ent = EntSchema {
//                 name: schema.name,
//                 anims: schema.anims,
//                 resource: schema.resource,
//                 albedo,
//                 normals,
//                 mesh,
//                 logic: schema.logic,
//                 resource_size: schema.resource_size,
//                 flat: true,
//             };
//         }

//         println!("loaded entity as {}", ent.name);
//         ent_map.insert(ent.name.to_owned(), ent);
//     }
//     let default_ent_schema = EntSchema {
//         name: String::from("NA"),
//         anims: HashMap::new(),
//         resource: String::from("none"),
//         albedo: Texture2D::empty(),
//         normals: Texture2D::empty(),
//         mesh: vec![Mesh {
//             vertices: [].to_vec(),
//             indices: [].to_vec(),
//             texture: Some(Texture2D::empty()),
//         }],
//         resource_size: [32, 32, 0].to_vec(),
//         logic: "".to_string(),
//         flat: false,
//     };
//     EntFactory {
//         ent_map,
//         default_ent_schema,
//         //lua_core: LuaCore::new(self),
//     }
// }

// fn log(str: String) {
//     crate::log(format!("ent_manager::", str));
// }
struct LuaEntMan {}
impl UserData for LuaEntMan {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(_methods: &mut M) {
        //TODO should we allow table field to not return nil? why?
        // methods.add_meta_method_mut("__index", |lua, this, ()| {
        //     //test
        //     Ok(())
        // });

        // methods.add_method("add", |lu, this, ()| {
        //     let ents = lu.globals().get::<&str, mlua::Table>("_ents")?;
        //     ents.set(this.get_id(), this);
        //     // this.get_id();
        //     Ok(())
        // });
        // methods.add_method("get_y", |_, this, ()| Ok(this.y));
    }
}

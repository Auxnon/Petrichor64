use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
};

use crate::{
    lua_ent::{lua_ent_flags, LuaEnt},
    model::{Model, ModelManager},
};

#[cfg(feature = "headed")]
use crate::{
    ent::{Ent, EntityUniforms},
    model::Instance,
    texture::TexManager,
};

use glam::{vec3, vec4};
use mlua::{UserData, UserDataMethods};
use rustc_hash::FxHashMap;
#[cfg(feature = "headed")]
use wgpu::{util::DeviceExt, Buffer};

#[cfg(feature = "headed")]
pub type InstanceBuffer = Vec<(Rc<Model>, Buffer, usize)>;
pub struct EntManager {
    #[cfg(feature = "headed")]
    pub specks: Vec<Ent>,
    // pub create: Vec<LuaEnt>,
    #[cfg(feature = "headed")]
    pub ent_array: Vec<(Arc<Mutex<LuaEnt>>, Ent, Rc<RefCell<EntityUniforms>>)>,
    #[cfg(not(feature = "headed"))]
    pub ent_array: Vec<Arc<Mutex<LuaEnt>>>,
    pub uniform_alignment: u32,
    #[cfg(feature = "headed")]
    pub instances: Vec<Instance>,
    #[cfg(feature = "headed")]
    pub instance_buffer: Buffer,
    pub id_counter: u64,
    #[cfg(feature = "headed")]
    pub render_hash: FxHashMap<String, (Rc<Model>, Vec<Rc<RefCell<EntityUniforms>>>)>,
    // pub render_pairs: Vec<(Arc<Mutex<LuaEnt>>, Rc<RefCell<Ent>>)>,
    pub hash_dirty: bool,
}

// (lua, ent)
impl EntManager {
    pub fn new(#[cfg(feature = "headed")] device: &wgpu::Device) -> EntManager {
        EntManager {
            #[cfg(feature = "headed")]
            specks: vec![],
            ent_array: vec![],
            #[cfg(feature = "headed")]
            instances: vec![],
            #[cfg(feature = "headed")]
            instance_buffer: EntManager::build_buffer(&vec![], device),
            uniform_alignment: 0,
            id_counter: 2,
            #[cfg(feature = "headed")]
            render_hash: FxHashMap::default(),
            // render_pairs: vec![],
            hash_dirty: false,
        }
    }

    #[cfg(feature = "headed")]
    pub fn build_buffer(instances: &Vec<Instance>, device: &wgpu::Device) -> Buffer {
        let instance_data = instances.iter().map(Instance::to_raw).collect::<Vec<_>>();
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(&instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    #[cfg(feature = "headed")]
    pub fn rebuild_instance_buffer(&mut self, device: &wgpu::Device) {
        self.instance_buffer = EntManager::build_buffer(&self.instances, device);
    }

    #[cfg(feature = "headed")]
    pub fn build_instance_buffer(
        instance_data: &Vec<EntityUniforms>,
        device: &wgpu::Device,
    ) -> Buffer {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Instance Buffer"),
            contents: bytemuck::cast_slice(instance_data),
            usage: wgpu::BufferUsages::VERTEX,
        })
    }

    #[cfg(feature = "headed")]
    pub fn create_from_lua(
        &mut self,
        tex_manager: &TexManager,
        model_manager: &ModelManager,
        wrapped_lua: Arc<Mutex<LuaEnt>>,
    ) {
        let lua = wrapped_lua.lock().unwrap();
        let id = lua.get_id();
        let mut asset = lua.get_asset();
        if asset.len() == 0 {
            asset = "example".to_string();
        }

        // MARK should change plane to a model if the texture doesn't exist as one
        let ent = Ent::new_dynamic(
            tex_manager,
            model_manager,
            vec3(lua.x as f32, lua.y as f32, lua.z as f32),
            0.,
            lua.scale as f32,
            0.,
            asset,
            self.uniform_alignment * (id + 1) as u32,
        );
        let uni = Rc::new(RefCell::new(ent.get_uniform(&lua, 0, None)));

        drop(lua);
        self.ent_array.push((wrapped_lua, ent, uni));
        self.hash_dirty = true
    }

    #[cfg(not(feature = "headed"))]
    pub fn create_from_lua(&mut self, wrapped_lua: Arc<Mutex<LuaEnt>>) {
        self.ent_array.push(wrapped_lua);
        self.hash_dirty = true
    }

    /** Set child as having parent.
     * Locate and lock the lua ent in iteration, then ensure the parent is located earlier on the array.
     *  Will reorder by placing the parent earlier on the array, just before the child.
     * Any existing children of that parent will still process correctly as they should already be further down the array having checked the order before.
     * It is possible to get some bad ordering if a user decides not to group in some sensible hiearchical order */
    pub fn group(&mut self, targetId: u64, childId: u64) {
        let mut parentIndex = -1;
        let mut childIndex = -1;
        for (i, lent) in self.ent_array.iter().enumerate() {
            #[cfg(feature = "headed")]
            let ll = lent.0.lock();
            #[cfg(not(feature = "headed"))]
            let ll = lent.lock();
            match ll {
                Ok(mut l) => {
                    let id = l.get_id();
                    if id == childId {
                        childIndex = i as i64;
                        if parentIndex != -1 {
                            l.parent = Some(targetId);
                            break;
                        }
                    } else if id == targetId {
                        parentIndex = i as i64;
                        if childIndex != -1 {
                            #[cfg(feature = "headed")]
                            let t = self.ent_array[childIndex as usize].0.lock();
                            #[cfg(not(feature = "headed"))]
                            let t = self.ent_array[childIndex as usize].lock();
                            t.unwrap().parent = Some(targetId);
                            break;
                        }
                    }
                }
                _ => {}
            }
        }
        if parentIndex != -1 && childIndex != -1 {
            if childIndex < parentIndex {
                let parent = self.ent_array.remove(parentIndex as usize);
                self.ent_array.insert(childIndex as usize, parent);
            }
        }
    }

    #[cfg(feature = "headed")]
    fn rebuild_render_hash(&mut self) {
        self.render_hash.clear();
        for (lent, ent, uni_ref) in &mut self.ent_array.iter() {
            match self.render_hash.get_mut(&ent.model.name) {
                Some((_, vec)) => {
                    vec.push(Rc::clone(uni_ref));
                }
                _ => {
                    self.render_hash.insert(
                        ent.model.name.clone(),
                        (Rc::clone(&ent.model), vec![Rc::clone(uni_ref)]),
                    );
                }
            }
        }
    }

    //

    #[cfg(feature = "headed")]
    pub fn tick_update_ents(
        &self,
        iteration: u64,
        device: &wgpu::Device,
    ) -> Vec<(Rc<Model>, Buffer, usize)> {
        let mut mats: FxHashMap<u64, glam::Mat4> = FxHashMap::default();
        for (alent, ent, uni_ref) in self.ent_array.iter() {
            if let Ok(lent) = alent.lock() {
                let parent = match lent.parent {
                    Some(u) => mats.get(&u),
                    None => None,
                };

                let mat = ent.build_meta(&lent, parent);
                let uni = ent.get_uniforms_with_mat(&lent, iteration, mat);
                uni_ref.replace(uni);
                mats.insert(lent.get_id(), mat);
            }
        }

        let instance_buffers = self
            .render_hash
            .iter()
            .map(|(name, (m, unis))| {
                let u = unis.iter().map(|u| u.borrow().clone()).collect::<Vec<_>>();
                let sz = u.len();
                (
                    Rc::clone(m),
                    crate::ent_manager::EntManager::build_instance_buffer(&u, device),
                    sz,
                )
            })
            .collect::<Vec<_>>();
        instance_buffers
    }

    // pub fn awful_test(&mut self, tex_manager: &TexManager, model_manager: &ModelManager) {
    //     let first = self.ent_table.len() as u64;
    //     for i in 0..100000 {
    //         let id = first + i;
    //         let p = vec3(
    //             -50. + rand::random::<f32>() * 100.,
    //             -50. + rand::random::<f32>() * 100.,
    //             -5. + rand::random::<f32>() * 10.,
    //         );
    //         let mut ent = Ent::new_dynamic(
    //             tex_manager,
    //             model_manager,
    //             p,
    //             0.,
    //             1.,
    //             0.,
    //             "zom".to_string(),
    //             self.uniform_alignment * (id + 1) as u32,
    //         );
    //         ent.pos = Some(p);
    //         self.specks.push(ent);
    //     }
    //     println!("awful test run {}", self.entities.len());
    // }

    pub fn reset(&mut self) {
        self.ent_array.clear();
        #[cfg(feature = "headed")]
        self.specks.clear();
    }

    pub fn reset_by_bundle(&mut self, bundle_id: u8) {
        println!(
            "looking for {}, ent count before bundle purge {}",
            bundle_id,
            self.ent_array.len()
        );
        #[cfg(feature = "headed")]
        self.ent_array.retain(|(le, e, u)| match le.lock() {
            Ok(lent) => {
                if lent.bundle_id == bundle_id {
                    false
                } else {
                    true
                }
            }
            _ => false,
        });
        #[cfg(not(feature = "headed"))]
        self.ent_array.retain(|le| match le.lock() {
            Ok(lent) => {
                if lent.bundle_id == bundle_id {
                    false
                } else {
                    true
                }
            }
            _ => false,
        });
        println!("ent count after bundle purge {}", self.ent_array.len());
    }

    #[cfg(not(feature = "headed"))]
    pub fn check_ents(&mut self, iteration: u64) {
        let mut mats: FxHashMap<u64, glam::Mat4> = FxHashMap::default();
        self.ent_array.retain_mut(|lent| {
            match lent.lock() {
                Ok(mut l) => {
                    let parent = match l.parent {
                        Some(u) => mats.get(&u),
                        None => None,
                    };
                    if l.is_dirty() {
                        let flags = l.get_flags();
                        if flags & lua_ent_flags::DEAD == lua_ent_flags::DEAD {
                            return false;
                        }
                        l.clear_dirt();
                    }
                    // let mat = ent.build_meta(&l, parent);
                    // mats.insert(l.get_id(), mat);
                }
                _ => {}
            }
            return true;
        });
    }

    #[cfg(feature = "headed")]
    pub fn check_ents(
        &mut self,
        device: &wgpu::Device,
        tm: &TexManager,
        mm: &ModelManager,
        iteration: u64,
    ) -> Vec<(Rc<Model>, Buffer, usize)> {
        let mut failed = 0;
        let mut mats: FxHashMap<u64, glam::Mat4> = FxHashMap::default();

        self.ent_array.retain_mut(|(lent, ent, uni_ref)| {
            match lent.lock() {
                Ok(mut l) => {
                    let parent = match l.parent {
                        Some(u) => mats.get(&u),
                        None => None,
                    };
                    if l.is_dirty() {
                        let flags = l.get_flags();

                        if flags & lua_ent_flags::DEAD == lua_ent_flags::DEAD {
                            self.hash_dirty = true;
                            return false;
                        }
                        l.clear_dirt();

                        if flags & lua_ent_flags::ASSET == lua_ent_flags::ASSET {
                            let asset = l.get_asset();
                            ent.model = Rc::clone(match mm.get_model_or_not(&asset) {
                                Some(m) => {
                                    // billboard
                                    if asset == "plane" {
                                        ent.effects.x = 1.;
                                    } else {
                                        ent.effects.x = 0.;
                                    }

                                    ent.tex = vec4(0., 0., 1., 1.);
                                    m
                                }
                                None => {
                                    // println!("no model found for {}", asset);
                                    if let Some(t) = tm.get_tex_or_not(&asset) {
                                        ent.tex = t;
                                    }
                                    &mm.CUBE
                                }
                            });
                            self.hash_dirty = true;
                        }

                        if flags & lua_ent_flags::TEX == lua_ent_flags::TEX {
                            ent.tex = tm.get_tex(l.get_tex());
                            ent.remove_anim();
                        }

                        if l.is_anim() {
                            let t = l.get_tex();
                            match tm.animations.get(t) {
                                Some(t) => {
                                    ent.set_anim(t.clone(), iteration);
                                }
                                _ => {}
                            }
                        }
                    } else {
                        // println!("not dirty");
                    }
                    let mat = ent.build_meta(&l, parent);
                    let uni = ent.get_uniforms_with_mat(&l, iteration, mat);
                    uni_ref.replace(uni);
                    mats.insert(l.get_id(), mat);
                }
                _ => {
                    failed += 1;
                }
            }
            return true;
        });

        #[cfg(feature = "headed")]
        let instance_buffers = self
            .render_hash
            .iter()
            .map(|(name, (m, unis))| {
                let u = unis.iter().map(|u| u.borrow().clone()).collect::<Vec<_>>();
                let sz = u.len();
                (
                    Rc::clone(m),
                    crate::ent_manager::EntManager::build_instance_buffer(&u, device),
                    sz,
                )
            })
            .collect::<Vec<_>>();

        if failed > 0 {
            println!("failed to lock {} ents", failed);
        }

        #[cfg(feature = "headed")]
        if self.hash_dirty {
            self.rebuild_render_hash();
            self.hash_dirty = false;
        }
        #[cfg(feature = "headed")]
        instance_buffers

        // if (self.specks.len() == 0 && self.specks.len() < 10000) {
        //     self.awful_test(tm, mm);
        // } else {
        //     for s in self.specks.iter_mut() {
        //         match s.pos {
        //             Some(mut p) => {
        //                 p.z += 0.1;
        //             }
        //             _ => {}
        //         }
        //     }
        // }
    }

    #[cfg(feature = "headed")]
    pub fn check_for_model_change(&mut self, model_manager: &ModelManager, model: &str) {
        let mut change = false;
        for (_, e, _) in self.ent_array.iter_mut() {
            if e.model.base_name == model {
                e.model = model_manager.get_model(model);
                change = true;
            }
        }
        if change {
            self.hash_dirty = true;
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

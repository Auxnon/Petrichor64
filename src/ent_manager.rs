
use crate::lua_ent::{lua_ent_flags, LuaEnt};

#[cfg(feature = "headed")]
use std::{cell::RefCell, rc::Rc};

#[cfg(feature = "headed")]
use crate::{
    ent::{Ent, EntityUniforms},
    model::{Instance, Model, ModelManager},
    texture::TexManager,
};

#[cfg(feature = "headed")]
use glam::{vec3, vec4};
#[cfg(feature = "puc_lua")]
use mlua::{UserData, UserDataMethods};
// Needed by the per-bundle entity map in both builds, headed or not.
use rustc_hash::FxHashMap;
use silt_lua::userdata::UserDataWrapper;
#[cfg(feature = "headed")]
use wgpu::{util::DeviceExt, Buffer};

#[cfg(feature = "headed")]
pub type InstanceBuffer = Vec<(Rc<Model>, Buffer, usize)>;

/// The Lua-side entity a render slot points at. On native it's the VM's
/// `UserDataWrapper` (shared with the Lua thread, so edits propagate for free).
/// On wasm the VM lives in a web worker, so the main thread can't hold a wrapper
/// — it keeps a `LuaEnt` mirror, refreshed by worker messages. `with_ref`/
/// `with_mut` give both a uniform accessor so the render code is target-agnostic.
#[cfg(all(feature = "headed", not(target_arch = "wasm32")))]
pub struct EntRef(pub UserDataWrapper);
#[cfg(all(feature = "headed", target_arch = "wasm32"))]
pub struct EntRef(pub LuaEnt);

#[cfg(feature = "headed")]
impl EntRef {
    pub fn with_ref<R>(
        &self,
        f: impl FnOnce(&LuaEnt) -> Result<R, silt_lua::LuaError>,
    ) -> Result<R, silt_lua::LuaError> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.0.downcast_ref::<LuaEnt, _, _>(f)
        }
        #[cfg(target_arch = "wasm32")]
        {
            f(&self.0)
        }
    }
    pub fn with_mut<R>(
        &mut self,
        f: impl FnOnce(&mut LuaEnt) -> Result<R, silt_lua::LuaError>,
    ) -> Result<R, silt_lua::LuaError>
    where
        // downcast_mut requires the closure's return type to be convertible
        // back into a Lua value; downcast_ref has no such bound, which is why
        // with_ref (above) doesn't need this.
        R: for<'a> silt_lua::value::ToLua<'a>,
    {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.0.downcast_mut::<LuaEnt, _, _>(f)
        }
        #[cfg(target_arch = "wasm32")]
        {
            f(&mut self.0)
        }
    }
}

pub struct EntManager {
    #[cfg(feature = "headed")]
    pub specks: Vec<Ent>,
    // pub create: Vec<LuaEnt>,
    /// Entities, owned per bundle. Keyed rather than held on `Bundle` itself so the
    /// entity types stay in this module; behaviourally it is the same ownership.
    pub bundles: FxHashMap<u8, BundleEnts>,
    pub uniform_alignment: u32,
    #[cfg(feature = "headed")]
    pub instances: Vec<Instance>,
    #[cfg(feature = "headed")]
    pub instance_buffer: Buffer,
    pub id_counter: u64,
    // pub render_pairs: Vec<(Arc<Mutex<LuaEnt>>, Rc<RefCell<Ent>>)>,
}

/// One bundle's entities and the render batches built from them.
///
/// Entities used to live in a single flat `Vec` shared by every bundle, which made
/// unloading an overlay a linear scan-and-filter over every entity in the engine, and
/// made one bundle spawning an entity rebuild *everyone's* batches. It also left no
/// way to draw "just this bundle", which an overlay with its own camera needs.
#[derive(Default)]
pub struct BundleEnts {
    #[cfg(feature = "headed")]
    pub array: Vec<(EntRef, Ent, Rc<RefCell<EntityUniforms>>)>,
    #[cfg(not(feature = "headed"))]
    pub array: Vec<UserDataWrapper>,
    #[cfg(feature = "headed")]
    pub render_hash: FxHashMap<String, (Rc<Model>, Vec<Rc<RefCell<EntityUniforms>>>)>,
    pub hash_dirty: bool,
}

#[cfg(feature = "headed")]
impl BundleEnts {
    fn rebuild_render_hash(&mut self) {
        self.render_hash.clear();
        for (_lent, ent, uni_ref) in self.array.iter() {
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

    fn instance_buffers(&self, device: &wgpu::Device) -> InstanceBuffer {
        self.render_hash
            .iter()
            .map(|(_name, (m, unis))| {
                let u = unis.iter().map(|u| u.borrow().clone()).collect::<Vec<_>>();
                let sz = u.len();
                (
                    Rc::clone(m),
                    EntManager::build_instance_buffer(&u, device),
                    sz,
                )
            })
            .collect::<Vec<_>>()
    }
}

// (lua, ent)
impl EntManager {
    pub fn new(#[cfg(feature = "headed")] device: &wgpu::Device) -> EntManager {
        EntManager {
            #[cfg(feature = "headed")]
            specks: vec![],
            bundles: FxHashMap::default(),
            #[cfg(feature = "headed")]
            instances: vec![],
            #[cfg(feature = "headed")]
            instance_buffer: EntManager::build_buffer(&vec![], device),
            uniform_alignment: 0,
            id_counter: 2,
            // render_pairs: vec![],
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

    // Native: the VM lives in-process, so we get the shared UserDataWrapper.
    #[cfg(all(feature = "headed", not(target_arch = "wasm32")))]
    /// `bundle_id` comes from the packet the VM sent, not from the entity: `LuaEnt`
    /// hardcodes 0 and nothing ever set it, so every entity in the engine claimed to
    /// belong to bundle 0. That silently broke `reset_by_bundle` long before this map
    /// existed — unloading bundle 0 purged *every* bundle's entities and unloading any
    /// other purged none. Stamping it here makes the owner authoritative (the sender's
    /// id) rather than something Lua could get wrong or lie about.
    pub fn create_from_lua(
        &mut self,
        tex_manager: &TexManager,
        model_manager: &ModelManager,
        bundle_id: u8,
        wrapped_lua: UserDataWrapper,
    ) {
        let (ent, uni) = wrapped_lua
            .downcast_ref::<LuaEnt, _, _>(|lent| {
                Ok(Self::build_ent(tex_manager, model_manager, lent, self.uniform_alignment))
            })
            .unwrap(); // should be safe since no errors within our closure

        let mut stamped = wrapped_lua;
        let _ = stamped.downcast_mut(|l: &mut LuaEnt| {
            l.bundle_id = bundle_id;
            Ok(())
        });
        let b = self.bundles.entry(bundle_id).or_default();
        b.array.push((EntRef(stamped), ent, uni));
        b.hash_dirty = true
    }

    // wasm: the VM is in a web worker; a Spawn message delivers a LuaEnt mirror
    // directly (no wrapper to downcast). Build the render Ent from it.
    #[cfg(all(feature = "headed", target_arch = "wasm32"))]
    pub fn create_from_lua_ent(
        &mut self,
        tex_manager: &TexManager,
        model_manager: &ModelManager,
        lent: LuaEnt,
    ) {
        let (ent, uni) = Self::build_ent(tex_manager, model_manager, &lent, self.uniform_alignment);
        let b = self.bundles.entry(lent.bundle_id).or_default();
        b.array.push((EntRef(lent), ent, uni));
        b.hash_dirty = true
    }

    /// Build the render `Ent` + its uniform cell from a `LuaEnt`. Shared by the
    /// native (wrapper) and wasm (mirror) spawn paths.
    #[cfg(feature = "headed")]
    fn build_ent(
        tex_manager: &TexManager,
        model_manager: &ModelManager,
        lent: &LuaEnt,
        uniform_alignment: u32,
    ) -> (Ent, Rc<RefCell<EntityUniforms>>) {
        let id = lent.get_id();
        let mut asset = lent.get_asset();
        if asset.is_empty() {
            asset = "example".to_string();
        }
        // MARK should change plane to a model if the texture doesn't exist as one
        let ent = Ent::new_dynamic(
            tex_manager,
            model_manager,
            vec3(lent.x as f32, lent.y as f32, lent.z as f32),
            0.,
            lent.scale as f32,
            0.,
            asset,
            uniform_alignment * (id + 1) as u32,
        );
        let uni = Rc::new(RefCell::new(ent.get_uniform(lent, 0, None)));
        (ent, uni)
    }

    #[cfg(not(feature = "headed"))]
    pub fn create_from_lua(&mut self, bundle_id: u8, mut wrapped_lua: UserDataWrapper) {
        let _ = wrapped_lua.downcast_mut(|l: &mut LuaEnt| {
            l.bundle_id = bundle_id;
            Ok(())
        });
        let b = self.bundles.entry(bundle_id).or_default();
        b.array.push(wrapped_lua);
        b.hash_dirty = true
    }

    /** Set child as having parent.
     * Locate and lock the lua ent in iteration, then ensure the parent is located earlier on the array.
     *  Will reorder by placing the parent earlier on the array, just before the child.
     * Any existing children of that parent will still process correctly as they should already be further down the array having checked the order before.
     * It is possible to get some bad ordering if a user decides not to group in some sensible hiearchical order */
    /// Set child as having parent, and make sure the parent is processed first.
    ///
    /// Scoped to the bundle that owns the child: parent matrices are resolved during
    /// that bundle's own pass, so a parent in another bundle could never be found
    /// anyway. Cross-bundle attachment is meant to go through `app.*` instead (an
    /// overlay reads the target's transform and positions its own entity).
    pub fn group(&mut self, target_id: u64, child_id: u64) {
        for b in self.bundles.values_mut() {
            let mut child_index: i64 = -1;
            let mut parent_index: i64 = -1;
            for (i, e) in b.array.iter().enumerate() {
                #[cfg(feature = "headed")]
                let id = e.0.with_ref(|l: &LuaEnt| Ok(l.get_id())).unwrap_or(u64::MAX);
                #[cfg(not(feature = "headed"))]
                let id = e
                    .downcast_ref::<LuaEnt, _, _>(|l| Ok(l.get_id()))
                    .unwrap_or(u64::MAX);
                if id == child_id {
                    child_index = i as i64;
                } else if id == target_id {
                    parent_index = i as i64;
                }
            }
            if child_index < 0 {
                continue;
            }

            #[cfg(feature = "headed")]
            let _ = b.array[child_index as usize].0.with_mut(|e| {
                e.parent = Some(target_id);
                Ok(())
            });
            #[cfg(not(feature = "headed"))]
            let _ = b.array[child_index as usize].downcast_mut::<LuaEnt, _, _>(|e| {
                e.parent = Some(target_id);
                Ok(())
            });

            // The parent's matrix has to exist before the child reads it, and that is
            // decided by position in this vec.
            if parent_index > child_index {
                let parent = b.array.remove(parent_index as usize);
                b.array.insert(child_index as usize, parent);
            }
            return;
        }
    }

    pub fn reset(&mut self) {
        self.bundles.clear();
        #[cfg(feature = "headed")]
        self.specks.clear();
    }

    /// Unloading a bundle is now dropping its entities, where it used to be a
    /// scan-and-filter of every entity in the engine.
    pub fn reset_by_bundle(&mut self, bundle_id: u8) {
        if let Some(b) = self.bundles.remove(&bundle_id) {
            println!("purged {} ents with bundle {}", b.array.len(), bundle_id);
        }
    }

    /// Visit every entity in every bundle. For the wasm paths, which address
    /// entities by id and don't care which bundle they came from.
    #[cfg(feature = "headed")]
    pub fn each_ent<F: FnMut(&mut EntRef)>(&mut self, mut f: F) {
        for b in self.bundles.values_mut() {
            for (eref, _ent, _uni) in b.array.iter_mut() {
                f(eref);
            }
        }
    }

    /// Drop entities whose id the predicate rejects, marking only the bundles that
    /// actually lost one as needing a batch rebuild.
    #[cfg(feature = "headed")]
    pub fn retain_ents<F: Fn(u64) -> bool>(&mut self, keep: F) {
        for b in self.bundles.values_mut() {
            let before = b.array.len();
            b.array.retain(|(eref, _ent, _uni)| {
                let mut k = true;
                let _ = eref.with_ref(|l| {
                    if !keep(l.get_id()) {
                        k = false;
                    }
                    Ok(())
                });
                k
            });
            if b.array.len() != before {
                b.hash_dirty = true;
            }
        }
    }

    #[cfg(not(feature = "headed"))]
    pub fn check_ents(&mut self, _iteration: u64) {
        // Headless has no GPU instances to build; just reap dead entities and
        // clear their dirty flags so per-frame Lua mutations settle.
        for b in self.bundles.values_mut() {
        b.array.retain_mut(|lent| {
            let mut alive = true;
            let _ = lent.downcast_mut::<LuaEnt, _, _>(|l| {
                if l.is_dirty() {
                    if l.get_flags() & lua_ent_flags::DEAD == lua_ent_flags::DEAD {
                        alive = false;
                    }
                    l.clear_dirt();
                }
                Ok(())
            });
            alive
        });
        }
    }

    #[cfg(feature = "headed")]
    /// Advance and rebuild one bundle's entities. Takes the bundle out of the map so
    /// the retain closure can still touch `self` for the shared managers.
    fn check_bundle_ents(
        &mut self,
        b: &mut BundleEnts,
        device: &wgpu::Device,
        tm: &TexManager,
        mm: &ModelManager,
        iteration: u64,
    ) -> InstanceBuffer {
        let mut failed = 0;
        let mut mats: FxHashMap<u64, glam::Mat4> = FxHashMap::default();

        b.array.retain_mut(|(lent, ent, uni_ref)| {
            if let Err(_) = lent.with_mut(|l: &mut LuaEnt| {
                let parent = match l.parent {
                    Some(u) => mats.get(&u),
                    None => None,
                };
                if l.is_dirty() {
                    let flags = l.get_flags();

                    if flags & lua_ent_flags::DEAD == lua_ent_flags::DEAD {
                        b.hash_dirty = true;
                        return Ok(false);
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
                                if let Some(t) = tm.get_tex_or_not(&asset) {
                                    ent.tex = t;
                                }
                                &mm.CUBE
                            }
                        });
                        b.hash_dirty = true;
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
                Ok(true)
            }) {
                failed += 1;
            }
            return true;
        });

        if failed > 0 {
            println!("failed to lock {} ents", failed);
        }

        // Rebuild the batches *before* reading them. This used to build the buffers
        // from `render_hash` and only then rebuild it, so the frame an entity spawned
        // or died drew the previous frame's set.
        if b.hash_dirty {
            b.rebuild_render_hash();
            b.hash_dirty = false;
        }
        b.instance_buffers(device)
    }

    /// Advance every bundle's entities and hand back one batch set per bundle, for
    /// the renderer to walk in `layer_order()`.
    #[cfg(feature = "headed")]
    pub fn check_ents(
        &mut self,
        device: &wgpu::Device,
        tm: &TexManager,
        mm: &ModelManager,
        iteration: u64,
    ) -> FxHashMap<u8, InstanceBuffer> {
        let ids: Vec<u8> = self.bundles.keys().copied().collect();
        let mut out = FxHashMap::default();
        for id in ids {
            // Taken out and put back so the per-bundle pass can hold `&mut self` for
            // the texture/model managers without aliasing the map.
            if let Some(mut b) = self.bundles.remove(&id) {
                let buffers = self.check_bundle_ents(&mut b, device, tm, mm, iteration);
                out.insert(id, buffers);
                self.bundles.insert(id, b);
            }
        }
        out
    }

    #[allow(dead_code)]
    fn _unused_specks_note(&self) {
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
    /// A model was reloaded: repoint every entity using it, and mark only the bundles
    /// that actually had one so the others keep their batches.
    pub fn check_for_model_change(&mut self, model_manager: &ModelManager, model: &str) {
        for b in self.bundles.values_mut() {
            let mut change = false;
            for (_, e, _) in b.array.iter_mut() {
                if e.model.base_name == model {
                    e.model = model_manager.get_model(model);
                    change = true;
                }
            }
            if change {
                b.hash_dirty = true;
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
// struct LuaEntMan {}
// impl UserData for LuaEntMan {
//     fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(_methods: &mut M) {
//         //TODO should we allow table field to not return nil? why?
//         // methods.add_meta_method_mut("__index", |lua, this, ()| {
//         //     //test
//         //     Ok(())
//         // });

//         // methods.add_method("add", |lu, this, ()| {
//         //     let ents = lu.globals().get::<&str, mlua::Table>("_ents")?;
//         //     ents.set(this.get_id(), this);
//         //     // this.get_id();
//         //     Ok(())
//         // });
//         // methods.add_method("get_y", |_, this, ()| Ok(this.y));
//     }
// }

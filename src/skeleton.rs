//! Skeletal animation: skins and clips parsed from glTF, and pose evaluation.
//!
//! A `Skeleton` is pure data (no GPU handles) so it can live in the shared
//! registry the Lua thread reads (and, on the web, be posted to the VM worker).
//! Lua owns each entity's animation state, the same way it owns the entity's
//! transform: it advances the clip time, applies bone overrides, and produces
//! the joint palette the renderer skins with. That keeps `ent:bone(...)` reads
//! exact and synchronous.
//!
//! Conventions:
//! - Vertex data is used as authored, like the static model path: no Y-up to
//!   Z-up conversion. Export skinned models Z-up (Blender: untick "+Y Up").
//! - A skin without inverse bind matrices gets them computed from the rest
//!   pose. The glTF default (identity) would displace every vertex by its
//!   joint's rest transform, which is never what an exporter that omits them
//!   means.
//! - Non-joint nodes between joints (e.g. an "Armature" parent) are folded
//!   into each joint's `link` matrix, so they still move the skeleton.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use glam::{EulerRot, Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// Upper bound on joints per skin, fixed by the shader's palette array.
pub const MAX_JOINTS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trs {
    pub t: [f32; 3],
    /// quaternion x, y, z, w
    pub r: [f32; 4],
    pub s: [f32; 3],
}

impl Trs {
    pub const IDENTITY: Trs = Trs {
        t: [0., 0., 0.],
        r: [0., 0., 0., 1.],
        s: [1., 1., 1.],
    };
    pub fn matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(
            Vec3::from(self.s),
            Quat::from_array(self.r),
            Vec3::from(self.t),
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Joint {
    pub name: String,
    /// Index of the nearest ancestor that is also a joint of this skin.
    pub parent: Option<usize>,
    /// Product of any non-joint node transforms between `parent` (or the
    /// scene root) and this joint.
    pub link: [f32; 16],
    pub rest: Trs,
    pub inverse_bind: [f32; 16],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Path {
    Translation,
    Rotation,
    Scale,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Interp {
    Step,
    Linear,
    /// Cubic spline: `values` holds (in-tangent, value, out-tangent) triplets.
    Cubic,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Channel {
    pub joint: usize,
    pub path: Path,
    pub interp: Interp,
    pub times: Vec<f32>,
    /// Flattened components: 3 per key for T/S, 4 for R (x3 for cubic).
    pub values: Vec<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Clip {
    pub name: String,
    pub duration: f32,
    pub channels: Vec<Channel>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Skeleton {
    pub joints: Vec<Joint>,
    pub clips: Vec<Clip>,
}

/// A script's adjustment to one bone, applied on top of the animated pose in
/// the bone's local space: rotation multiplies in, translation adds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BoneOverride {
    pub rot: [f32; 3],
    pub pos: [f32; 3],
}

impl BoneOverride {
    pub fn quat(&self) -> Quat {
        Quat::from_euler(EulerRot::ZYX, self.rot[2], self.rot[1], self.rot[0])
    }
}

impl Skeleton {
    pub fn joint_index(&self, name: &str) -> Option<usize> {
        self.joints.iter().position(|j| j.name == name)
    }

    pub fn clip_index(&self, name: &str) -> Option<usize> {
        self.clips.iter().position(|c| c.name == name)
    }

    /// Local poses for `clip` at time `t` (seconds). Channels a clip doesn't
    /// animate keep the rest pose. `t` wraps when `looped`, clamps otherwise.
    pub fn sample(&self, clip: Option<usize>, t: f32, looped: bool) -> Vec<Trs> {
        let mut locals: Vec<Trs> = self.joints.iter().map(|j| j.rest).collect();
        let Some(clip) = clip.and_then(|c| self.clips.get(c)) else {
            return locals;
        };
        let t = if clip.duration <= 0. {
            0.
        } else if looped {
            t.rem_euclid(clip.duration)
        } else {
            t.clamp(0., clip.duration)
        };
        for ch in &clip.channels {
            let Some(local) = locals.get_mut(ch.joint) else {
                continue;
            };
            match ch.path {
                Path::Translation => local.t = sample_vec3(ch, t),
                Path::Scale => local.s = sample_vec3(ch, t),
                Path::Rotation => local.r = sample_quat(ch, t),
            }
        }
        locals
    }

    /// Apply overrides (by joint index) to sampled local poses.
    pub fn apply_overrides(locals: &mut [Trs], overrides: &[(usize, BoneOverride)]) {
        for (i, o) in overrides {
            if let Some(l) = locals.get_mut(*i) {
                let r = (Quat::from_array(l.r) * o.quat()).normalize();
                l.r = r.to_array();
                l.t = (Vec3::from(l.t) + Vec3::from(o.pos)).to_array();
            }
        }
    }

    /// Model-space transform of every joint. Parents always precede children
    /// in `joints` (enforced at load), so one forward pass suffices.
    pub fn globals(&self, locals: &[Trs]) -> Vec<Mat4> {
        let mut out: Vec<Mat4> = Vec::with_capacity(self.joints.len());
        for (i, j) in self.joints.iter().enumerate() {
            let parent = j.parent.map(|p| out[p]).unwrap_or(Mat4::IDENTITY);
            out.push(parent * Mat4::from_cols_array(&j.link) * locals[i].matrix());
        }
        out
    }

    /// Skinning matrices: global * inverse bind, one per joint.
    pub fn palette(&self, globals: &[Mat4]) -> Vec<Mat4> {
        self.joints
            .iter()
            .zip(globals)
            .map(|(j, g)| *g * Mat4::from_cols_array(&j.inverse_bind))
            .collect()
    }
}

fn key_span(times: &[f32], t: f32) -> (usize, usize, f32) {
    if times.len() < 2 || t <= times[0] {
        return (0, 0, 0.);
    }
    let last = times.len() - 1;
    if t >= times[last] {
        return (last, last, 0.);
    }
    // first key strictly after t
    let hi = times.partition_point(|&k| k <= t);
    let lo = hi - 1;
    let span = times[hi] - times[lo];
    let f = if span > 0. { (t - times[lo]) / span } else { 0. };
    (lo, hi, f)
}

fn read<const N: usize>(ch: &Channel, key: usize) -> [f32; N] {
    // cubic keys store (in, value, out); the value is the middle one
    let (stride, off) = match ch.interp {
        Interp::Cubic => (N * 3, N),
        _ => (N, 0),
    };
    let mut out = [0.; N];
    let base = key * stride + off;
    out.copy_from_slice(&ch.values[base..base + N]);
    out
}

fn sample_vec3(ch: &Channel, t: f32) -> [f32; 3] {
    let (lo, hi, f) = key_span(&ch.times, t);
    let a = Vec3::from(read::<3>(ch, lo));
    if ch.interp == Interp::Step || lo == hi {
        return a.to_array();
    }
    let b = Vec3::from(read::<3>(ch, hi));
    // Cubic is approximated linearly between the key values: tangents are
    // rarely meaningful in console-scale rigs.
    a.lerp(b, f).to_array()
}

fn sample_quat(ch: &Channel, t: f32) -> [f32; 4] {
    let (lo, hi, f) = key_span(&ch.times, t);
    let a = Quat::from_array(read::<4>(ch, lo)).normalize();
    if ch.interp == Interp::Step || lo == hi {
        return a.to_array();
    }
    let b = Quat::from_array(read::<4>(ch, hi)).normalize();
    a.slerp(b, f).to_array()
}

/// Per-vertex skin data, parallel to a model's vertex array.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkinVertex {
    pub joints: [u16; 4],
    pub weights: [f32; 4],
}

/// Bytes of one instance's palette in the per-batch uniform buffer: the
/// shader's fixed `array<mat4x4<f32>, 64>`, which is also a multiple of the
/// 256-byte dynamic offset alignment.
pub const PALETTE_STRIDE: u64 = (MAX_JOINTS * 64) as u64;

#[cfg(feature = "headed")]
impl SkinVertex {
    const ATTRIBS: [wgpu::VertexAttribute; 2] =
        wgpu::vertex_attr_array![11 => Uint16x4, 12 => Float32x4];
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<SkinVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBS,
        }
    }
}

/// Bind group 3 of the skinned pipelines: one instance's joint palette,
/// selected with a dynamic offset into the batch's palette buffer.
#[cfg(feature = "headed")]
pub fn palette_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("skin palette layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: wgpu::BufferSize::new(PALETTE_STRIDE),
            },
            count: None,
        }],
    })
}

/// Build a `Skeleton` from a glTF skin. Joints are reordered so every parent
/// precedes its children; `remap[gltf_joint_slot]` gives the new index, which
/// vertex JOINTS_0 values must be translated through.
pub fn from_gltf(
    doc: &gltf::Document,
    skin: &gltf::Skin,
    buffers: &[gltf::buffer::Data],
) -> Result<(Skeleton, Vec<u16>), String> {
    let skin_nodes: Vec<usize> = skin.joints().map(|n| n.index()).collect();
    if skin_nodes.len() > MAX_JOINTS {
        return Err(format!(
            "skin has {} joints, the limit is {}",
            skin_nodes.len(),
            MAX_JOINTS
        ));
    }

    let node_count = doc.nodes().count();
    let mut parent_of: Vec<Option<usize>> = vec![None; node_count];
    for n in doc.nodes() {
        for c in n.children() {
            parent_of[c.index()] = Some(n.index());
        }
    }
    let local_of = |i: usize| -> Trs {
        let (t, r, s) = doc.nodes().nth(i).unwrap().transform().decomposed();
        Trs { t, r, s }
    };

    // Parent-first ordering: sort joints by depth in the node tree.
    let depth = |mut i: usize| {
        let mut d = 0;
        while let Some(p) = parent_of[i] {
            d += 1;
            i = p;
        }
        d
    };
    let mut order: Vec<usize> = (0..skin_nodes.len()).collect();
    order.sort_by_key(|&slot| depth(skin_nodes[slot]));
    let mut remap = vec![0u16; skin_nodes.len()];
    for (new, &slot) in order.iter().enumerate() {
        remap[slot] = new as u16;
    }
    let slot_of_node: HashMap<usize, usize> =
        skin_nodes.iter().enumerate().map(|(s, &n)| (n, s)).collect();

    let ibms: Option<Vec<[[f32; 4]; 4]>> = skin
        .reader(|b| Some(&buffers[b.index()]))
        .read_inverse_bind_matrices()
        .map(|m| m.collect());

    let mut joints = Vec::with_capacity(order.len());
    for &slot in &order {
        let node = skin_nodes[slot];
        // Walk up to the nearest joint ancestor, folding non-joint nodes in.
        let mut link = Mat4::IDENTITY;
        let mut parent = None;
        let mut cur = parent_of[node];
        while let Some(p) = cur {
            if let Some(&ps) = slot_of_node.get(&p) {
                parent = Some(remap[ps] as usize);
                break;
            }
            link = local_of(p).matrix() * link;
            cur = parent_of[p];
        }
        let n = doc.nodes().nth(node).unwrap();
        joints.push(Joint {
            name: n.name().map(str::to_owned).unwrap_or_else(|| format!("joint{}", node)),
            parent,
            link: link.to_cols_array(),
            rest: local_of(node),
            inverse_bind: ibms
                .as_ref()
                .map(|m| Mat4::from_cols_array_2d(&m[slot]).to_cols_array())
                .unwrap_or_else(|| Mat4::IDENTITY.to_cols_array()),
        });
    }

    let mut skel = Skeleton {
        joints,
        clips: vec![],
    };
    if ibms.is_none() {
        let rest: Vec<Trs> = skel.joints.iter().map(|j| j.rest).collect();
        let g = skel.globals(&rest);
        for (j, m) in skel.joints.iter_mut().zip(g) {
            j.inverse_bind = m.inverse().to_cols_array();
        }
    }

    for anim in doc.animations() {
        let mut channels = vec![];
        let mut duration = 0f32;
        for ch in anim.channels() {
            let Some(&slot) = slot_of_node.get(&ch.target().node().index()) else {
                continue;
            };
            let reader = ch.reader(|b| Some(&buffers[b.index()]));
            let Some(times) = reader.read_inputs().map(|i| i.collect::<Vec<f32>>()) else {
                continue;
            };
            use gltf::animation::util::ReadOutputs;
            let (path, values): (Path, Vec<f32>) = match reader.read_outputs() {
                Some(ReadOutputs::Translations(v)) => (Path::Translation, v.flatten().collect()),
                Some(ReadOutputs::Scales(v)) => (Path::Scale, v.flatten().collect()),
                Some(ReadOutputs::Rotations(v)) => {
                    (Path::Rotation, v.into_f32().flatten().collect())
                }
                _ => continue,
            };
            let interp = match ch.sampler().interpolation() {
                gltf::animation::Interpolation::Step => Interp::Step,
                gltf::animation::Interpolation::Linear => Interp::Linear,
                gltf::animation::Interpolation::CubicSpline => Interp::Cubic,
            };
            if let Some(&last) = times.last() {
                duration = duration.max(last);
            }
            channels.push(Channel {
                joint: remap[slot] as usize,
                path,
                interp,
                times,
                values,
            });
        }
        if !channels.is_empty() {
            skel.clips.push(Clip {
                name: anim
                    .name()
                    .map(|n| n.to_lowercase())
                    .unwrap_or_else(|| format!("clip{}", anim.index())),
                duration,
                channels,
            });
        }
    }
    Ok((skel, remap))
}

/// Engine time in seconds, published by the main loop each frame so the Lua
/// thread and the renderer sample clips against the same clock.
static CLOCK: AtomicU64 = AtomicU64::new(0);

pub fn set_clock(secs: f64) {
    CLOCK.store(secs.to_bits(), Ordering::Relaxed);
}

pub fn clock() -> f64 {
    f64::from_bits(CLOCK.load(Ordering::Relaxed))
}

/// An entity's skeletal animation state, owned by its `LuaEnt`. The pose is a
/// pure function of this, the skeleton and the clock, so the renderer and
/// `ent:bone()` always agree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SkelState {
    /// Playing clip; empty means the rest pose.
    pub clip: String,
    /// `clock()` when the clip started.
    pub start: f64,
    pub speed: f32,
    pub looped: bool,
    pub bones: Vec<(String, BoneOverride)>,
}

impl Default for SkelState {
    fn default() -> Self {
        SkelState {
            clip: String::new(),
            start: 0.,
            speed: 1.,
            looped: true,
            bones: vec![],
        }
    }
}

impl SkelState {
    /// Model-space joint transforms at `now`.
    pub fn globals(&self, skel: &Skeleton, now: f64) -> Vec<Mat4> {
        let clip = if self.clip.is_empty() {
            None
        } else {
            skel.clip_index(&self.clip)
        };
        let t = ((now - self.start) * self.speed as f64) as f32;
        let mut locals = skel.sample(clip, t, self.looped);
        let overrides: Vec<(usize, BoneOverride)> = self
            .bones
            .iter()
            .filter_map(|(n, o)| skel.joint_index(n).map(|i| (i, *o)))
            .collect();
        Skeleton::apply_overrides(&mut locals, &overrides);
        skel.globals(&locals)
    }

    pub fn set_bone(&mut self, name: &str, o: Option<BoneOverride>) {
        self.bones.retain(|(n, _)| n != name);
        if let Some(o) = o {
            self.bones.push((name.to_owned(), o));
        }
    }

    pub fn bone(&self, name: &str) -> Option<BoneOverride> {
        self.bones.iter().find(|(n, _)| n == name).map(|(_, o)| *o)
    }
}

/// Skinning matrices in the renderer's packed space: vertex positions and
/// entity translations are both scaled by 16, so each matrix is conjugated by
/// that scale and applies directly to packed positions.
pub fn palette16(skel: &Skeleton, globals: &[Mat4]) -> Vec<Mat4> {
    let up = Mat4::from_scale(Vec3::splat(16.));
    let down = Mat4::from_scale(Vec3::splat(1. / 16.));
    skel.palette(globals)
        .into_iter()
        .map(|p| up * p * down)
        .collect()
}

/// Register a model file's skeleton without building any GPU mesh, for builds
/// with no device (headless, terminal) so `ent:bone()` still works there.
/// `name` is the model's base name (file stem).
pub fn register_from_bytes(name: &str, bytes: &[u8]) -> Result<(), String> {
    let (doc, buffers, _) = gltf::import_slice(bytes).map_err(|e| e.to_string())?;
    let Some((skin, mesh)) = doc.nodes().find_map(|n| Some((n.skin()?, n.mesh()?))) else {
        return Ok(());
    };
    let (skel, _) = from_gltf(&doc, &skin, &buffers)?;
    let base = name.to_lowercase();
    if let Some(mesh_name) = mesh.name() {
        register(&format!("{}.{}", base, mesh_name.to_lowercase()), skel.clone());
    }
    register(&base, skel);
    Ok(())
}

lazy_static::lazy_static! {
    /// Skeletons by model name, readable from the Lua thread. The model loader
    /// fills it; on the web the main thread forwards entries to the worker.
    pub static ref SKELETONS: RwLock<HashMap<String, Arc<Skeleton>>> = RwLock::new(HashMap::new());
}

pub fn register(name: &str, skel: Skeleton) {
    if let Ok(mut m) = SKELETONS.write() {
        m.insert(name.to_lowercase(), Arc::new(skel));
    }
}

/// Model names are stored lowercased, as the model dictionary does.
/// Registry entries beyond the first `sent`, for forwarding to the web VM
/// worker; bumps `sent`. Entries are only ever added, never replaced in bulk.
pub fn take_new(sent: &mut usize) -> Vec<(String, Skeleton)> {
    let Ok(m) = SKELETONS.read() else {
        return vec![];
    };
    if m.len() <= *sent {
        return vec![];
    }
    *sent = m.len();
    m.iter().map(|(k, v)| (k.clone(), (**v).clone())).collect()
}

pub fn lookup(name: &str) -> Option<Arc<Skeleton>> {
    SKELETONS
        .read()
        .ok()
        .and_then(|m| m.get(&name.to_lowercase()).cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_bone() -> Skeleton {
        // root at origin, child one unit up Z; rest = bind
        let mut s = Skeleton {
            joints: vec![
                Joint {
                    name: "root".into(),
                    parent: None,
                    link: Mat4::IDENTITY.to_cols_array(),
                    rest: Trs::IDENTITY,
                    inverse_bind: Mat4::IDENTITY.to_cols_array(),
                },
                Joint {
                    name: "tip".into(),
                    parent: Some(0),
                    link: Mat4::IDENTITY.to_cols_array(),
                    rest: Trs {
                        t: [0., 0., 1.],
                        ..Trs::IDENTITY
                    },
                    inverse_bind: Mat4::from_translation(Vec3::new(0., 0., -1.)).to_cols_array(),
                },
            ],
            clips: vec![],
        };
        let q = Quat::from_rotation_x(std::f32::consts::FRAC_PI_2);
        s.clips.push(Clip {
            name: "bend".into(),
            duration: 2.,
            channels: vec![Channel {
                joint: 0,
                path: Path::Rotation,
                interp: Interp::Linear,
                times: vec![0., 2.],
                values: [Quat::IDENTITY.to_array(), q.to_array()].concat(),
            }],
        });
        s
    }

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-4
    }

    #[test]
    fn rest_pose_palette_is_identity() {
        let s = two_bone();
        let pal = s.palette(&s.globals(&s.sample(None, 0., true)));
        for m in pal {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-5));
        }
    }

    #[test]
    fn rotation_propagates_to_children() {
        let s = two_bone();
        // halfway: 45 degrees about X, so the tip (0,0,1) swings toward -Y
        let g = s.globals(&s.sample(Some(0), 1., true));
        let tip = g[1].transform_point3(Vec3::ZERO);
        let h = std::f32::consts::FRAC_1_SQRT_2;
        assert!(close(tip, Vec3::new(0., -h, h)), "{:?}", tip);
    }

    #[test]
    fn time_wraps_when_looped_and_clamps_otherwise() {
        let s = two_bone();
        let a = s.sample(Some(0), 0.5, true);
        let b = s.sample(Some(0), 2.5, true);
        assert_eq!(a, b);
        let end = s.sample(Some(0), 2., false);
        assert_eq!(s.sample(Some(0), 9., false), end);
    }

    #[test]
    fn step_holds_the_previous_key() {
        let mut s = two_bone();
        s.clips[0].channels[0].interp = Interp::Step;
        let a = s.sample(Some(0), 1.9, true);
        assert_eq!(a[0].r, Quat::IDENTITY.to_array());
    }

    #[test]
    fn overrides_compose_on_top_of_the_clip() {
        let s = two_bone();
        let mut locals = s.sample(None, 0., true);
        let o = BoneOverride {
            rot: [std::f32::consts::FRAC_PI_2, 0., 0.],
            pos: [0., 0., 0.],
        };
        Skeleton::apply_overrides(&mut locals, &[(0, o)]);
        let tip = s.globals(&locals)[1].transform_point3(Vec3::ZERO);
        assert!(close(tip, Vec3::new(0., -1., 0.)), "{:?}", tip);
    }

    #[test]
    fn parses_a_real_blender_rig() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/test/skin/player.glb");
        let (doc, buffers, _) = gltf::import(path).expect("fixture loads");
        let skin = doc.skins().next().expect("has a skin");
        let (s, remap) = from_gltf(&doc, &skin, &buffers).expect("skin parses");
        assert_eq!(s.joints.len(), 21);
        assert_eq!(remap.len(), 21);
        // parents precede children
        for (i, j) in s.joints.iter().enumerate() {
            if let Some(p) = j.parent {
                assert!(p < i, "{} before its parent", j.name);
            }
        }
        let head = s.joint_index("head").unwrap();
        let neck = s.joint_index("neck").unwrap();
        assert_eq!(s.joints[head].parent, Some(neck));
        let mut names: Vec<&str> = s.clips.iter().map(|c| c.name.as_str()).collect();
        names.sort();
        assert_eq!(names, ["carry", "idle", "sit"]);
        // no inverse binds in this file: computed, so the rest pose is unskinned
        let rest = s.sample(None, 0., true);
        for m in s.palette(&s.globals(&rest)) {
            assert!(m.abs_diff_eq(Mat4::IDENTITY, 1e-4));
        }
        // This rig's clips are single-frame poses: idle is the rest pose, carry
        // and sit hold a pose. So idle stays put and the other two move.
        for c in &s.clips {
            let ci = s.clip_index(&c.name);
            let mut most = 0f32;
            for k in 0..8 {
                let t = c.duration * k as f32 / 8.;
                for m in s.palette(&s.globals(&s.sample(ci, t, true))) {
                    most = most.max(m.transform_point3(Vec3::Y).distance(Vec3::Y));
                }
            }
            if c.name == "idle" {
                assert!(most < 1e-3, "idle should be the rest pose, moved {}", most);
            } else {
                assert!(most > 0.01, "clip {} doesn't move", c.name);
            }
        }
    }

    #[test]
    fn missing_inverse_binds_come_from_the_rest_pose() {
        let mut s = two_bone();
        for j in &mut s.joints {
            j.inverse_bind = Mat4::IDENTITY.to_cols_array();
        }
        let rest: Vec<Trs> = s.joints.iter().map(|j| j.rest).collect();
        let g = s.globals(&rest);
        for (j, m) in s.joints.iter_mut().zip(g) {
            j.inverse_bind = m.inverse().to_cols_array();
        }
        let pal = s.palette(&s.globals(&rest));
        assert!(pal[1].abs_diff_eq(Mat4::IDENTITY, 1e-5));
    }
}

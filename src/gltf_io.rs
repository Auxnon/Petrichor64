//! glTF in and out as plain mesh arrays — the `glb` native's backend.
//!
//! The engine already *loads* glTF as models (`model.rs`), but that path bakes the
//! mesh into packed `i16` vertices on the GPU and keeps no float copy, so it can't
//! be read back or written out. This module works on the same arrays `mod()` takes
//! (positions, normals, uvs, 0-based indices) so a Lua tool can build a mesh, show
//! it with `mod()`, and save it with `glb()` from one set of tables.
//!
//! Export writes a single mesh/primitive, flat-out: no node hierarchy, no skins.
//! A texture, if given, is embedded as PNG with a nearest-filtered sampler, which is
//! what the engine's own loader reads back (it pulls every image out of the file
//! into the atlas). Positions are written as-is, Z-up like the rest of the engine;
//! `MeshData::to_y_up`/`from_y_up` exist for files headed to Y-up tools.

use image::RgbaImage;
use serde_json::{json, Value};

/// What the engine signs its own glTF files with; a file carrying it is one
/// mesh and at most one image, so it can be rewritten without losing anything.
pub const GENERATOR: &str = "Petrichor64 glb()";

#[derive(Default, Debug, Clone, PartialEq)]
pub struct MeshData {
    pub pos: Vec<[f32; 3]>,
    pub norm: Vec<[f32; 3]>,
    pub uv: Vec<[f32; 2]>,
    /// 0-based, three per triangle — the same convention as `mod()`'s `i`.
    pub ind: Vec<u32>,
}

impl MeshData {
    /// Z-up (engine) to Y-up (glTF's spec convention): (x, y, z) -> (x, z, -y).
    /// A rotation, so winding and normals stay valid.
    pub fn to_y_up(&mut self) {
        for p in self.pos.iter_mut().chain(self.norm.iter_mut()) {
            *p = [p[0], p[2], -p[1]];
        }
    }

    /// Inverse of [`MeshData::to_y_up`].
    pub fn from_y_up(&mut self) {
        for p in self.pos.iter_mut().chain(self.norm.iter_mut()) {
            *p = [p[0], -p[2], p[1]];
        }
    }

    /// Fill in anything an exporter needs that the caller left out: missing
    /// indices become a plain triangle list, missing normals are computed flat per
    /// triangle (shared vertices get the sum, normalized), missing uvs are zero.
    /// Returns an error for data that can't form triangles at all.
    pub fn complete(&mut self) -> Result<(), String> {
        let n = self.pos.len();
        if n < 3 {
            return Err("glb: need at least 3 vertices".into());
        }
        if self.ind.is_empty() {
            self.ind = (0..(n - n % 3) as u32).collect();
        }
        if self.ind.len() % 3 != 0 {
            return Err(format!(
                "glb: {} indices is not a whole number of triangles",
                self.ind.len()
            ));
        }
        if let Some(bad) = self.ind.iter().find(|&&i| i as usize >= n) {
            return Err(format!("glb: index {} out of range for {} vertices", bad, n));
        }
        if self.norm.len() != n {
            self.norm = flat_normals(&self.pos, &self.ind);
        }
        if self.uv.len() != n {
            self.uv = vec![[0., 0.]; n];
        }
        Ok(())
    }
}

pub(crate) fn flat_normals(pos: &[[f32; 3]], ind: &[u32]) -> Vec<[f32; 3]> {
    let mut out = vec![glam::Vec3::ZERO; pos.len()];
    for tri in ind.chunks_exact(3) {
        let [a, b, c] = [tri[0], tri[1], tri[2]].map(|i| glam::Vec3::from(pos[i as usize]));
        let n = (b - a).cross(c - a);
        for &i in tri {
            out[i as usize] += n;
        }
    }
    out.into_iter()
        .map(|n| n.try_normalize().unwrap_or(glam::Vec3::Z).to_array())
        .collect()
}

/// Newell's method: a stable normal for any polygon, including a slightly
/// non-planar one, oriented by the right-hand rule over its winding.
fn polygon_normal(pos: &[[f32; 3]], face: &[u32]) -> glam::Vec3 {
    let mut n = glam::Vec3::ZERO;
    for (k, &i) in face.iter().enumerate() {
        let a = glam::Vec3::from(pos[i as usize]);
        let b = glam::Vec3::from(pos[face[(k + 1) % face.len()] as usize]);
        n += glam::vec3(
            (a.y - b.y) * (a.z + b.z),
            (a.z - b.z) * (a.x + b.x),
            (a.x - b.x) * (a.y + b.y),
        );
    }
    n.try_normalize().unwrap_or(glam::Vec3::Z)
}

impl MeshData {
    /// Flat-shaded triangles from polygons — the `mod()`/`glb()` polygon form.
    /// `faces` hold 0-based indices into `pos`, wound so the right-hand rule
    /// points out; each face is fanned from its first vertex (exact for convex
    /// faces) and gets its own copies of its vertices, carrying the face normal
    /// and `face_uv[face]` (a texel to colour it from; 0,0 if absent).
    pub fn from_polygons(pos: &[[f32; 3]], faces: &[Vec<u32>], face_uv: &[[f32; 2]]) -> Result<Self, String> {
        let mut m = MeshData::default();
        for (fi, face) in faces.iter().enumerate() {
            if face.len() < 3 {
                continue;
            }
            if let Some(bad) = face.iter().find(|&&i| i as usize >= pos.len()) {
                return Err(format!("face {} uses vertex {} of {}", fi + 1, bad + 1, pos.len()));
            }
            let n = polygon_normal(pos, face).to_array();
            let uv = face_uv.get(fi).copied().unwrap_or([0., 0.]);
            let base = m.pos.len() as u32;
            for &i in face {
                m.pos.push(pos[i as usize]);
                m.norm.push(n);
                m.uv.push(uv);
            }
            for k in 1..face.len() as u32 - 1 {
                m.ind.extend_from_slice(&[base, base + k, base + k + 1]);
            }
        }
        Ok(m)
    }

    /// Square beams along line segments — the `mod()` line form, for wireframes,
    /// gizmos and debug lines. `pts` are endpoint pairs; each beam is `width` thick
    /// (per segment, or the last width given) and textured from `seg_uv`. Four
    /// long sides and no end caps: a beam this thin never shows its ends.
    pub fn from_lines(pts: &[[f32; 3]], widths: &[f32], seg_uv: &[[f32; 2]]) -> Self {
        let mut m = MeshData::default();
        for (si, seg) in pts.chunks_exact(2).enumerate() {
            let a = glam::Vec3::from(seg[0]);
            let b = glam::Vec3::from(seg[1]);
            let Some(dir) = (b - a).try_normalize() else { continue };
            let w = widths.get(si).or(widths.last()).copied().unwrap_or(0.02);
            let reference = if dir.z.abs() > 0.9 { glam::Vec3::X } else { glam::Vec3::Z };
            let s1 = dir.cross(reference).normalize();
            let s2 = dir.cross(s1);
            let (u, v) = (s1 * w, s2 * w);
            let ring = [u + v, -u + v, -u - v, u - v];
            let uv = seg_uv.get(si).copied().unwrap_or([0., 0.]);
            for k in 0..4 {
                let (c0, c1) = (ring[k], ring[(k + 1) % 4]);
                let n = (c0 + c1).normalize().to_array();
                let base = m.pos.len() as u32;
                for p in [a + c0, b + c0, b + c1, a + c1] {
                    m.pos.push(p.to_array());
                    m.norm.push(n);
                    m.uv.push(uv);
                }
                m.ind.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
            }
        }
        m
    }
}

/// The binary payload (vertex attributes then indices, each 4-byte aligned) and
/// the matching glTF JSON, before either container wraps them.
fn build(mesh: &MeshData, image: Option<&RgbaImage>, uri: Option<&dyn Fn(&[u8]) -> String>) -> Result<(Value, Vec<u8>), String> {
    let mut bin: Vec<u8> = vec![];
    let mut views: Vec<Value> = vec![];

    let mut push_view = |bin: &mut Vec<u8>, bytes: &[u8], target: Option<u32>| -> usize {
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        let offset = bin.len();
        bin.extend_from_slice(bytes);
        let mut v = json!({ "buffer": 0, "byteOffset": offset, "byteLength": bytes.len() });
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        views.push(v);
        views.len() - 1
    };

    let f3 = |a: &[[f32; 3]]| -> Vec<u8> { a.iter().flatten().flat_map(|f| f.to_le_bytes()).collect() };
    let f2 = |a: &[[f32; 2]]| -> Vec<u8> { a.iter().flatten().flat_map(|f| f.to_le_bytes()).collect() };

    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in &mesh.pos {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }

    const ARRAY_BUFFER: u32 = 34962;
    const ELEMENT_ARRAY_BUFFER: u32 = 34963;
    const FLOAT: u32 = 5126;
    const UNSIGNED_INT: u32 = 5125;
    const NEAREST: u32 = 9728;

    let pv = push_view(&mut bin, &f3(&mesh.pos), Some(ARRAY_BUFFER));
    let nv = push_view(&mut bin, &f3(&mesh.norm), Some(ARRAY_BUFFER));
    let uvv = push_view(&mut bin, &f2(&mesh.uv), Some(ARRAY_BUFFER));
    let ind_bytes: Vec<u8> = mesh.ind.iter().flat_map(|i| i.to_le_bytes()).collect();
    let iv = push_view(&mut bin, &ind_bytes, Some(ELEMENT_ARRAY_BUFFER));

    let count = mesh.pos.len();
    let accessors = json!([
        { "bufferView": pv, "componentType": FLOAT, "count": count, "type": "VEC3", "min": lo, "max": hi },
        { "bufferView": nv, "componentType": FLOAT, "count": count, "type": "VEC3" },
        { "bufferView": uvv, "componentType": FLOAT, "count": count, "type": "VEC2" },
        { "bufferView": iv, "componentType": UNSIGNED_INT, "count": mesh.ind.len(), "type": "SCALAR" },
    ]);

    let mut material = json!({
        "name": "material",
        "pbrMetallicRoughness": { "baseColorFactor": [1.0, 1.0, 1.0, 1.0], "metallicFactor": 0.0, "roughnessFactor": 1.0 },
    });

    let mut root = json!({
        "asset": { "version": "2.0", "generator": GENERATOR },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "name": "mesh" }],
        "meshes": [{
            "name": "mesh",
            "primitives": [{
                "attributes": { "POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2 },
                "indices": 3,
                "material": 0,
            }],
        }],
        "accessors": accessors,
    });

    if let Some(img) = image {
        let mut png: Vec<u8> = vec![];
        image::DynamicImage::ImageRgba8(img.clone())
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .map_err(|e| format!("glb: png encode failed: {}", e))?;
        let image_json = match uri {
            // .gltf: the image rides as its own data uri, like the buffer.
            Some(to_uri) => json!({ "uri": to_uri(&png), "mimeType": "image/png" }),
            None => {
                let v = push_view(&mut bin, &png, None);
                json!({ "bufferView": v, "mimeType": "image/png" })
            }
        };
        root["images"] = json!([image_json]);
        root["samplers"] = json!([{ "magFilter": NEAREST, "minFilter": NEAREST }]);
        root["textures"] = json!([{ "source": 0, "sampler": 0 }]);
        material["pbrMetallicRoughness"]["baseColorTexture"] = json!({ "index": 0 });
    }
    root["materials"] = json!([material]);

    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    root["bufferViews"] = Value::Array(views);
    Ok((root, bin))
}

/// Binary glTF (.glb): JSON chunk + BIN chunk in one file.
pub fn to_glb(mesh: &MeshData, image: Option<&RgbaImage>) -> Result<Vec<u8>, String> {
    let (mut root, bin) = build(mesh, image, None)?;
    root["buffers"] = json!([{ "byteLength": bin.len() }]);
    let mut js = serde_json::to_vec(&root).map_err(|e| e.to_string())?;
    while js.len() % 4 != 0 {
        js.push(b' '); // the spec pads the JSON chunk with spaces
    }
    let total = 12 + 8 + js.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(js.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&js);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    Ok(out)
}

/// Text glTF (.gltf) with the buffer and image embedded as base64 data uris, so
/// it's still one self-contained file.
pub fn to_gltf(mesh: &MeshData, image: Option<&RgbaImage>) -> Result<String, String> {
    let to_uri = |b: &[u8]| format!("data:image/png;base64,{}", base64(b));
    let (mut root, bin) = build(mesh, image, Some(&to_uri))?;
    root["buffers"] = json!([{
        "byteLength": bin.len(),
        "uri": format!("data:application/octet-stream;base64,{}", base64(&bin)),
    }]);
    serde_json::to_string_pretty(&root).map_err(|e| e.to_string())
}

/// Read every triangle primitive of every mesh in a glTF/glb into one `MeshData`,
/// indices rebased as primitives are appended. Node transforms are ignored, the
/// same as the engine's model loader, so what you read is what `mod()` would show.
pub fn read(bytes: &[u8]) -> Result<MeshData, String> {
    let (doc, buffers, _) = gltf::import_slice(bytes).map_err(|e| e.to_string())?;
    let mut out = MeshData::default();
    let mut missing_normals = false;
    for mesh in doc.meshes() {
        for prim in mesh.primitives() {
            if prim.mode() != gltf::mesh::Mode::Triangles {
                continue;
            }
            let reader = prim.reader(|b| Some(&buffers[b.index()]));
            let pos: Vec<[f32; 3]> = match reader.read_positions() {
                Some(p) => p.collect(),
                None => continue,
            };
            let base = out.pos.len() as u32;
            let n = pos.len();
            let norm: Vec<[f32; 3]> = reader.read_normals().map(|i| i.collect()).unwrap_or_default();
            let uv: Vec<[f32; 2]> = reader
                .read_tex_coords(0)
                .map(|t| t.into_f32().collect())
                .unwrap_or_default();
            let ind: Vec<u32> = match reader.read_indices() {
                Some(i) => i.into_u32().collect(),
                None => (0..n as u32).collect(),
            };
            out.pos.extend(pos);
            // Keep the attribute arrays aligned with positions even when a
            // primitive is missing one. Missing normals are recomputed for the
            // whole mesh afterwards rather than patched per primitive.
            if norm.len() == n {
                out.norm.extend(norm);
            } else {
                missing_normals = true;
                out.norm.resize(out.pos.len(), [0., 0., 0.]);
            }
            if uv.len() == n {
                out.uv.extend(uv);
            } else {
                out.uv.resize(out.pos.len(), [0., 0.]);
            }
            out.ind.extend(ind.into_iter().map(|i| i + base));
        }
    }
    if out.pos.is_empty() {
        return Err("glb: no triangle meshes in file".into());
    }
    if missing_normals {
        out.norm.clear();
    }
    out.complete()?;
    Ok(out)
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for c in bytes.chunks(3) {
        let b = [c[0], *c.get(1).unwrap_or(&0), *c.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        for k in 0..4 {
            if k <= c.len() {
                s.push(T[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                s.push('=');
            }
        }
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quad() -> MeshData {
        MeshData {
            pos: vec![[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]],
            norm: vec![],
            uv: vec![[0., 0.], [1., 0.], [1., 1.], [0., 1.]],
            ind: vec![0, 1, 2, 0, 2, 3],
        }
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn complete_fills_flat_normals_facing_ccw() {
        let mut m = quad();
        m.complete().unwrap();
        assert!(m.norm.iter().all(|n| *n == [0., 0., 1.]));
    }

    #[test]
    fn complete_rejects_bad_indices() {
        let mut m = quad();
        m.ind = vec![0, 1, 9];
        assert!(m.complete().is_err());
        let mut m = quad();
        m.ind = vec![0, 1];
        assert!(m.complete().is_err());
    }

    #[test]
    fn glb_round_trips() {
        let mut m = quad();
        m.complete().unwrap();
        let mut img = RgbaImage::new(2, 2);
        img.put_pixel(1, 0, image::Rgba([255, 0, 0, 255]));
        let bytes = to_glb(&m, Some(&img)).unwrap();
        assert_eq!(&bytes[0..4], b"glTF");
        assert_eq!(bytes.len() % 4, 0);
        let back = read(&bytes).unwrap();
        assert_eq!(back, m);
        // and the engine's loader path (gltf::import_slice) sees the image
        let (_, _, images) = gltf::import_slice(&bytes).unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!((images[0].width, images[0].height), (2, 2));
    }

    #[test]
    fn gltf_text_round_trips() {
        let mut m = quad();
        m.complete().unwrap();
        let text = to_gltf(&m, None).unwrap();
        let back = read(text.as_bytes()).unwrap();
        assert_eq!(back, m);
    }

    #[test]
    fn polygons_fan_and_carry_face_normals() {
        let pos = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.], [0., 0., 1.]];
        let faces = vec![vec![0, 1, 2, 3], vec![0, 4, 1]];
        let m = MeshData::from_polygons(&pos, &faces, &[[0.5, 0.25]]).unwrap();
        assert_eq!(m.pos.len(), 7); // 4 + 3, unshared
        assert_eq!(m.ind, vec![0, 1, 2, 0, 2, 3, 4, 5, 6]);
        assert_eq!(m.norm[0], [0., 0., 1.]);
        assert_eq!(m.norm[4], [0., 1., 0.]); // (z then x): right-hand rule gives +y
        assert_eq!(m.uv[0], [0.5, 0.25]);
        assert_eq!(m.uv[4], [0., 0.]); // no uv given for face 2
        let bad = MeshData::from_polygons(&pos, &[vec![0, 1, 9]], &[]);
        assert!(bad.is_err());
    }

    #[test]
    fn lines_make_four_sided_beams() {
        let m = MeshData::from_lines(&[[0., 0., 0.], [2., 0., 0.], [0., 0., 0.], [0., 0., 0.]], &[0.1], &[]);
        // the zero-length second segment is skipped
        assert_eq!(m.pos.len(), 16);
        assert_eq!(m.ind.len(), 24);
        for p in &m.pos {
            let off = (p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((off - 0.1 * 2f32.sqrt()).abs() < 1e-5);
        }
    }

    #[test]
    fn y_up_is_reversible() {
        let mut m = quad();
        m.complete().unwrap();
        let orig = m.clone();
        m.to_y_up();
        assert_eq!(m.norm[0], [0., 1., 0.]);
        m.from_y_up();
        assert_eq!(m, orig);
    }
}

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::template::AssetTemplate;
use glam::{vec4, UVec2, UVec4, Vec4};
use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect, draw_filled_rect_mut};
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use parking_lot::{Mutex, RwLock};
use rustc_hash::FxHashMap;
use wgpu::{Queue, Sampler, Texture, TextureView};

#[cfg(target_os = "windows")]
const SLASH: char = '\\';
#[cfg(not(target_os = "windows"))]
const SLASH: char = '/';

lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref MASTER: Arc<Mutex<OnceCell<RgbaImage>>> = Arc::new(Mutex::new(OnceCell::new()));
    /** Last position of a locatated section for a texture, x y, the last   */
    pub static ref ATLAS_POS:Mutex<UVec4> = Mutex::new(UVec4::new(0,0,0,0));
    /** Current dimensions of the atlas image, likely stays the same in most cases */
    pub static ref ATLAS_DIM:Mutex<UVec2>= Mutex::new(UVec2::new(0,0));
    /** Our wonderful string to uv coordinate map, give us a texture name and we'll give you a position on the atlas of that texture! */
    pub static ref DICTIONARY:RwLock<HashMap<String,Vec4>> =RwLock::new(HashMap::new());
    /** a really basic UUID, just incrementing from 0..n is fine internally, can we even go higher then 4294967295 (2^32 -1 is u32 max)?*/
    pub static ref COUNTER:Mutex<u32> =Mutex::new(2);
    /** map our texture strings to their integer value for fast lookup and 3d grid insertion*/
    pub static ref INT_DICTIONARY:RwLock<HashMap<String,u32>> = RwLock::new(HashMap::new());
    pub static ref INT_MAP:RwLock<FxHashMap<u32,Vec4>> = RwLock::new(FxHashMap::default());
    pub static ref ANIMATIONS:RwLock<HashMap<String,(Vec<Vec4>,u32)>> = RwLock::new(HashMap::default());
}

pub fn init() {
    let img: RgbaImage = ImageBuffer::new(1024, 1024);
    let mut d = ATLAS_DIM.lock();
    d.x = 1024;
    d.y = 1024;
    MASTER.lock().get_or_init(|| img);
}

pub fn reset() {
    let img: RgbaImage = ImageBuffer::new(1024, 1024);
    let mut d = ATLAS_DIM.lock();
    d.x = 1024;
    d.y = 1024;
    let mut p = ATLAS_POS.lock();
    p.x = 0;
    p.y = 0;
    p.z = 0;
    p.w = 0;
    DICTIONARY.write().clear();
    *COUNTER.lock() = 2;
    INT_DICTIONARY.write().clear();
    INT_MAP.write().clear();

    match MASTER.lock().get_mut() {
        Some(im) => image::imageops::replace(im, &img, 0, 0),
        None => error("Somehow missing our texture atlas?".to_string()),
    }
}

pub fn save_atlas() {
    let mas = MASTER.lock();
    let buf = mas.get().unwrap();
    let dim = buf.dimensions();
    match image::save_buffer_with_format(
        "atlas.png",
        &buf,
        1024,
        1024,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    ) {
        Ok(_) => lg!("saved atlas {}x{}", dim.0, dim.1),
        Err(err) => lg!("failed to save atlas:{}", err),
    }
}

// fn draw_rect(img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>) {
//     let magenta = Rgba([255u8, 0u8, 255u8, 255u8]);

//     // imageproc::drawing::write_pixel(img, 0, 0, &magenta);
//     draw_filled_rect_mut(
//         img,
//         imageproc::rect::Rect::at(10, 10).of_size(75, 75),
//         magenta,
//     );
// }

pub fn simple_square(size: u32, path: PathBuf) {
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
        Err(err) => err!("could not save example image: {}", err),
        _ => {}
    }
}

//MARK save_audio_buffer
pub fn save_audio_buffer(buffer: &Vec<u8>) {
    // println!("🟣buffer {} {}", buffer.len(), 512);
    let w = buffer.len() as u32;
    let h = 512;
    let img: RgbaImage = ImageBuffer::new(w, h);

    let yellow = image::Rgba::from([255, 255, 0, 255]);
    let black = image::Rgba::from([0, 0, 0, 255]);
    let rect = imageproc::rect::Rect::at(0, 0).of_size(w, h);
    let mut new_img = draw_filled_rect(&img, rect, yellow);

    //  (&mut img, black);
    for (i, c) in buffer.iter().enumerate() {
        new_img.put_pixel(i as u32, *c as u32, black);
    }

    // let image_buffer = match image::RgbaImage::from_raw(w, h, img.to_vec()) {
    //     Some(o) => {
    // o.put_pixel(0,0, black);

    match image::save_buffer_with_format(
        "sound.png",
        &new_img,
        w,
        h,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    ) {
        Err(err) => {
            err!("unable to save image: {}", err);
        }
        _ => {}
    }
}

pub fn finalize(device: &wgpu::Device, queue: &Queue) -> (TextureView, Sampler, Texture) {
    make_tex(device, queue, MASTER.lock().get().unwrap())
}

pub fn refinalize(queue: &Queue, texture: &Texture) {
    write_tex(queue, texture, &MASTER.lock().get().unwrap());
}

pub fn render_sampler(device: &wgpu::Device, size: (u32, u32)) -> (TextureView, Sampler, Texture) {
    let img: RgbaImage = ImageBuffer::new(size.0, size.1);
    make_render_tex(device, &img)
}

/**locate a position in the  master texture atlas, return a v4 of the tex coord x y offset and the scaleX scaleY to multiply the uv by to get the intended texture */
pub fn locate(source: RgbaImage) -> Vec4 {
    let mut m_guard = MASTER.lock();
    let m_ref = m_guard.get_mut().unwrap();
    assert!(
        source.width() < m_ref.width() && source.height() < m_ref.height(),
        "Texture atlas isnt big enough for this image :("
    );
    let mut found = false;
    let mut apos = ATLAS_POS.lock();
    let mut cpos = apos.clone();
    let adim = ATLAS_DIM.lock();
    let w = source.width();
    let h = source.height();

    if apos.x + w <= adim.x && apos.y + h <= adim.y {
        found = true;
        apos.x += w;
    } else {
        if apos.x + w > adim.x {
            apos.x = w;
            apos.y += apos.w;
            cpos.x = 0;
            cpos.y = apos.y;
            found = true;
        } else if apos.y + h < adim.y {
            panic!("Texture atlas couldnt find an empty spot?");
        }
    }
    // if (apos.x != 0 && apos.y != 0) {
    //     //while (!found && apos.y < m_ref.height()) {

    //     if (apos.x + w) <= adim.x {
    //         found = true;
    //         apos.x += w;
    //     } else {
    //         apos.x = 0;
    //         apos.y += apos.w;
    //         cpos.x = 0;
    //         cpos.y = apos.y;
    //         if (apos.x + w) < adim.x && (apos.y + h) < adim.y {
    //             found = true;
    //         }
    //         apos.x += w;
    //     }

    //     //if()
    //     //}
    // } else {
    //     found = true;
    //     apos.x += w;
    // }
    if found && apos.w < h {
        apos.w = h;
    }

    assert!(found, "Texture atlas couldnt find an empty spot?");
    lg!("found position {} {}", cpos.x, cpos.y);
    stich(m_ref, source, cpos.x, cpos.y);
    Vec4::new(
        cpos.x as f32 / adim.x as f32,
        cpos.y as f32 / adim.y as f32,
        w as f32 / adim.x as f32,
        h as f32 / adim.y as f32,
    )
}

pub fn load_img(str: &String) -> Result<DynamicImage, image::ImageError> {
    let text = Path::new("assets").join(str).to_str().unwrap().to_string();
    //Path::new(".").join("entities");
    load_img_nopath(&text)
}
pub fn load_img_nopath(str: &String) -> Result<DynamicImage, image::ImageError> {
    lg!("{}", str.clone());

    println!("opening asset {}", str);

    let img = image::open(str);

    // The dimensions method returns the images width and height.
    //println!("dimensions height {:?}", img.height());

    // The color method returns the image's `ColorType`.
    //println!("{:?}", img.color());
    img
}
pub fn load_img_from_buffer(buffer: &[u8]) -> Result<DynamicImage, image::ImageError> {
    //Path::new(".").join("entities");
    //log(text.clone());

    let img = image::load_from_memory(buffer);
    // The dimensions method returns the images width and height.
    //println!("dimensions height {:?}", img.height());

    // The color method returns the image's `ColorType`.
    //println!("{:?}", img.color());
    img
}
fn tile_locate(
    name: String,
    dim: (u32, u32),
    pos: Vec4,
    mut tile_dim: u32,
    rename: Option<HashMap<u32, String>>,
) {
    if tile_dim == 0 {
        tile_dim = 16;
    }
    let dw = dim.0 / tile_dim;
    let dh = dim.1 / tile_dim;
    let iw = 1. / dw as f32;
    let ih = 1. / dh as f32;

    /*
    assert!(found, "Texture atlas couldnt find an empty spot?");
    log(format!("found position {} {}", cpos.x, cpos.y));
    stich(m_ref, source, cpos.x, cpos.y);
    cgmath::Vector4::new(
        cpos.x as f32 / adim.x as f32,
        cpos.y as f32 / adim.y as f32,
        w as f32 / adim.x as f32,
        h as f32 / adim.y as f32,
    ) */
    // println!(
    //     "d {} {} {} {} and seed pos {} {} {} {}",
    //     dw, dh, iw, ih, pos.x, pos.y, pos.z, pos.w
    // );
    let can_rename = if rename.is_some() { true } else { false };
    for y in 0..dh {
        for x in 0..dw {
            let n = x + (y * dw);
            let mut p = pos.clone();

            p.z *= iw as f32;
            p.w *= ih as f32;
            p.x += p.z * x as f32;
            p.y += p.w * y as f32;
            let key = format!("{}{}", name, n);
            // println!("🟠made texture {} at {}", key, p);
            // Key to our texture
            let index_key = index_texture(key.clone(), p);

            // log(format!(
            //     "made tile tex {} at {} {} {} {}",
            //     key, p.x, p.y, p.z, p.w
            // ));
            let mut dict = DICTIONARY.write();
            dict.insert(key, p);
            if can_rename {
                match rename {
                    Some(ref r) => {
                        match r.get(&n) {
                            Some(name) => {
                                dict.insert(name.clone(), p);
                                index_texture_direct(name.clone(), index_key);
                            }
                            _ => {}
                        };
                    }
                    _ => {}
                }
            }
        }
    }
    //dictionary.lock().insert(name, pos);
}

/** Create a simple numerical key for our texture and uv and map it, returning that numerical key*/
fn index_texture(key: String, p: Vec4) -> u32 {
    let mut guard = COUNTER.lock();
    let ind = *guard;
    INT_MAP.write().insert(ind, p);
    INT_DICTIONARY.write().insert(key, ind);
    *guard += 1;
    ind
}

/** We already had a numerical key created in a previous index_texture call and want to add another String->u32 translation*/
fn index_texture_direct(key: String, index: u32) {
    INT_DICTIONARY.write().insert(key, index);
}

pub fn load_tex_from_buffer(str: &String, buffer: &Vec<u8>, template: Option<&AssetTemplate>) {
    // println!("🟢load_tex_from_buffer{} is {}", str, buffer.len());
    match image::load_from_memory(buffer.as_slice()) {
        Ok(img) => sort_image(str, img, template, true),
        Err(err) => err!("failed to load texture {}", err),
    }
}

pub fn load_tex(str: &String, template: Option<&AssetTemplate>) {
    // println!("🟢load_tex{}", str);
    lg!("apply texture {}", str);

    match load_img_nopath(str) {
        Ok(img) => sort_image(str, img, template, false),
        Err(err) => {
            err!("failed to load texture {}", err);
            // dictionary
            //     .lock()
            //     .insert(name, cgmath::Vector4::new(0., 0., 0., 0.));
        }
    }
}

fn sort_image(
    str: &String,
    img: DynamicImage,
    template: Option<&AssetTemplate>,
    from_unpack: bool,
) {
    let (name, mut is_tile) = get_name(str.clone(), from_unpack);
    let mut rename = None;
    let mut tile_dim = 16u32;
    match template {
        Some(t) => {
            if t.tiles.len() > 0 || t.size > 0 {
                is_tile = true;
            }
            rename = Some(t.tiles.clone());
            tile_dim = t.size;
        }
        _ => {}
    }
    if is_tile {
        let dim = (img.width(), img.height());
        let pos = locate(img.into_rgba8());
        tile_locate(name, dim, pos, tile_dim, rename);
    } else {
        let pos = locate(img.into_rgba8());
        println!("sort_image name {} pos {}", name, pos);
        DICTIONARY.write().insert(name.clone(), pos);
        index_texture(name, pos);
    }
}

fn get_name(str: String, from_unpack: bool) -> (String, bool) {
    if from_unpack {
        //TODO we can assume hopefully that no file extension was provided in an unpack call of this func, this should be fixed but it's complicated
        let bits = str.split(".").collect::<Vec<_>>();
        match bits.get(0) {
            Some(o) => {
                if bits.len() > 1 && bits.get(bits.len() - 1).unwrap() == &"tile" {
                    return (o.to_string(), true);
                }
                (o.to_string(), false)
            }
            None => (str, false),
        }
    } else {
        let smol = str.split(SLASH).collect::<Vec<_>>();
        let bits = smol.last().unwrap().split(".").collect::<Vec<_>>();
        match bits.get(0) {
            Some(o) => {
                if bits.len() > 2 && bits.get(1).unwrap() == &"tile" {
                    return (o.to_string(), true);
                }
                (o.to_string(), false)
            }
            None => (str, false),
        }
    }
}

pub fn load_tex_from_img(short_name: String, path: String, im: &Vec<gltf::image::Data>) -> Vec4 {
    let pvec = im
        .iter()
        .flat_map(|d| d.pixels.as_slice().to_owned())
        .collect::<Vec<_>>();

    // println!("HI🟣🟣🟣🟣🟣🟣🟣");
    // let (actual_name, _) = get_name(str, false);

    let mut pos = Vec4::new(0., 0., 0., 0.);
    let image_buffer = match image::RgbaImage::from_raw(64, 64, pvec) {
        Some(o) => o,
        None => {
            error("Failed to load texture from mesh".to_string());
            DICTIONARY.write().insert(short_name, pos);
            return vec4(1., 1., 0., 0.);
        }
    };

    pos = locate(image_buffer);

    // println!("🔥inject image {} from buffe {}", short_name, pos);

    DICTIONARY.write().insert(short_name.clone(), pos);
    pos
    // index_texture(short_name, pos);
}

/** return texture uv coordinates from a given texture name */
pub fn get_tex(str: &String) -> Vec4 {
    match DICTIONARY.read().get(str) {
        Some(v) => v.clone(),
        None => Vec4::new(0., 0., 0., 0.),
    }
}

pub fn _list_keys() -> String {
    DICTIONARY
        .read()
        .keys()
        .map(|k| k.clone())
        .collect::<Vec<String>>()
        .join(",")
}

/** return texture numerical index from a given texture name */
pub fn get_tex_index(str: &String) -> u32 {
    match INT_DICTIONARY.read().get(str) {
        Some(n) => n.clone(),
        _ => 1,
    }
}
/** return texture uv coordinates and numerical index from a given texture name */
// pub fn get_tex_and_index(str: &String) {}

/** return texture uv coordinates from a given texture numerical index */
pub fn get_tex_from_index(ind: u32) -> Vec4 {
    match INT_MAP.read().get(&ind) {
        Some(uv) => uv.clone(),
        _ => vec4(1., 1., 0., 0.),
    }
}

pub fn stich(master_img: &mut RgbaImage, source: RgbaImage, x: u32, y: u32) {
    image::imageops::overlay(master_img, &source, x as i64, y as i64);
}

pub fn write_tex(queue: &Queue, texture: &Texture, img: &RgbaImage) {
    let dimensions = img.dimensions();

    let texture_size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };

    queue.write_texture(
        // Tells wgpu where to copy the pixel data
        wgpu::ImageCopyTexture {
            texture: texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        // The actual pixel data
        img,
        // The layout of the texture
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
            rows_per_image: std::num::NonZeroU32::new(dimensions.1),
        },
        texture_size,
    );
}

pub fn make_tex(
    device: &wgpu::Device,
    queue: &Queue,
    img: &RgbaImage,
) -> (TextureView, Sampler, Texture) {
    lg!("make master texture");
    let rgba = img; //img.as_rgba8().unwrap();
    let dimensions = img.dimensions();
    let texture_size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };

    let tex = device.create_texture(&wgpu::TextureDescriptor {
        // All textures are stored as 3D, we represent our 2D texture
        // by setting depth to 1.
        size: texture_size,
        mip_level_count: 1, // We'll talk about this a little later
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Most images are stored using sRGB so we need to reflect that here.
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        // TEXTURE_BINDING tells wgpu that we want to use this texture in shaders
        // COPY_DST means that we want to copy data to this texture
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: Some("diffuse_texture"),
    });

    queue.write_texture(
        // Tells wgpu where to copy the pixel data
        wgpu::ImageCopyTexture {
            texture: &tex,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        // The actual pixel data
        rgba,
        // The layout of the texture
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
            rows_per_image: std::num::NonZeroU32::new(dimensions.1),
        },
        texture_size,
    );
    let diffuse_texture_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    (diffuse_texture_view, diffuse_sampler, tex)
}

pub fn make_render_tex(
    device: &wgpu::Device,
    // queue: &Queue,
    img: &RgbaImage,
) -> (TextureView, Sampler, Texture) {
    lg!("make master texture");
    // let rgba = img; //img.as_rgba8().unwrap();
    let dimensions = img.dimensions();
    let texture_size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };

    let tex = device.create_texture(&wgpu::TextureDescriptor {
        // All textures are stored as 3D, we represent our 2D texture
        // by setting depth to 1.
        size: texture_size,
        mip_level_count: 1, // We'll talk about this a little later
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        // Most images are stored using sRGB so we need to reflect that here.
        format: wgpu::TextureFormat::Bgra8UnormSrgb,
        // TEXTURE_BINDING tells wgpu that we want to use this texture in shaders
        // COPY_DST means that we want to copy data to this texture
        usage: wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        // | wgpu::TextureUsages::COPY_SRC,
        label: Some("post_texture"),
    });

    // queue.write_texture(
    //     // Tells wgpu where to copy the pixel data
    //     wgpu::ImageCopyTexture {
    //         texture: &tex,
    //         mip_level: 0,
    //         origin: wgpu::Origin3d::ZERO,
    //         aspect: wgpu::TextureAspect::All,
    //     },
    //     // The actual pixel data
    //     rgba,
    //     // The layout of the texture
    //     wgpu::ImageDataLayout {
    //         offset: 0,
    //         bytes_per_row: std::num::NonZeroU32::new(4 * dimensions.0),
    //         rows_per_image: std::num::NonZeroU32::new(dimensions.1),
    //     },
    //     texture_size,
    // );
    let diffuse_texture_view = tex.create_view(&wgpu::TextureViewDescriptor::default());
    let diffuse_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        address_mode_u: wgpu::AddressMode::Repeat,
        address_mode_v: wgpu::AddressMode::Repeat,
        address_mode_w: wgpu::AddressMode::Repeat,
        mag_filter: wgpu::FilterMode::Nearest,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Nearest,
        ..Default::default()
    });
    (diffuse_texture_view, diffuse_sampler, tex)
}

pub fn set_anims(name: &String, frames: Vec<Vec4>, animation_speed: u32) {
    println!("set anims {} {:?}", name, frames);
    ANIMATIONS
        .write()
        .insert(name.clone(), (frames, animation_speed));
}

macro_rules! lg{
    ($($arg:tt)*) => {{
           {
            let st=format!("🎨texture::{}",format!($($arg)*));
            println!("{}",st);
            crate::log::log(st);
           }
       }
   }
}

macro_rules! err{
    ($($arg:tt)*) => {{
           {
            crate::log::error(format!("‼︎ERROR::🎨texture::{}",format!($($arg)*)));
           }
       }
   }
}
pub(crate) use err;
pub(crate) use lg;

fn error(str: String) {
    crate::log::error(format!("‼︎ERROR::🎨texture::{}", str));
}

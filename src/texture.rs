use std::{collections::HashMap, path::Path, sync::Arc};

use glam::{UVec2, UVec4, Vec4};
use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage};
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use wgpu::{util::DeviceExt, Queue, Sampler, Texture, TextureView};

lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref master: Arc<Mutex<OnceCell<RgbaImage>>> = Arc::new(Mutex::new(OnceCell::new()));
    /** Last position of a locatated section for a texture, x y, the last   */
    pub static ref atlas_pos:Mutex<UVec4> = Mutex::new(UVec4::new(0,0,0,0));
    /** Current dimensuons of the atlas image, likely stays the same in most cases */
    pub static ref atlas_dim:Mutex<UVec2>= Mutex::new(UVec2::new(0,0));
    pub static ref dictionary:Mutex<HashMap<String,Vec4>> =Mutex::new(HashMap::new());
}

pub fn init() {
    let mut img: RgbaImage = ImageBuffer::new(1024, 1024);
    let mut d = atlas_dim.lock();
    d.x = 1024;
    d.y = 1024;
    let rgba = master.lock().get_or_init(|| img);
}
pub fn reset() {
    let mut img: RgbaImage = ImageBuffer::new(1024, 1024);
    let mut d = atlas_dim.lock();
    d.x = 1024;
    d.y = 1024;
    match master.lock().get_mut() {
        Some(im) => image::imageops::replace(im, &img, 0, 0),
        None => error("Somehow missing our texture atlas?".to_string()),
    }
}
pub fn save_atlas() {
    let mas = master.lock();
    let buf = mas.get().unwrap();
    let dim = buf.dimensions();
    println!("atlas w {} h {}", dim.0, dim.1);
    // let v = buf.to_vec();
    // for b in v {
    //     print!("_{}", b);
    // }
    match image::save_buffer_with_format(
        "atlas.png",
        &buf,
        1024,
        1024,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    ) {
        Ok(s) => {}
        Err(err) => {}
    }
}
pub fn finalize(device: &wgpu::Device, queue: &Queue) -> (TextureView, Sampler, Texture) {
    make_tex(device, queue, master.lock().get().unwrap())
}

pub fn refinalize(device: &wgpu::Device, queue: &Queue, texture: &Texture) {
    write_tex(device, queue, texture, &master.lock().get().unwrap());
}
/**locate a position in the  master texture atlas, return a v4 of the tex coord x y offset and the scaleX scaleY to multiply the uv by to get the intended texture */
pub fn locate(source: RgbaImage) -> Vec4 {
    let mut m_guard = master.lock();
    let m_ref = m_guard.get_mut().unwrap();
    assert!(
        source.width() < m_ref.width() && source.height() < m_ref.height(),
        "Texture atlas isnt big enough for this image :("
    );
    let mut found = false;
    let mut apos = atlas_pos.lock();
    let mut cpos = apos.clone();
    let adim = atlas_dim.lock();
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
    log(format!("found position {} {}", cpos.x, cpos.y));
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
    log(text.clone());

    let img = image::open(text);

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
fn tile_locate(name: String, dim: (u32, u32), pos: Vec4) {
    let dw = dim.0 / 16;
    let dh = dim.1 / 16;
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
    for x in 0..dw {
        for y in 0..dh {
            let n = x + (y * dw);
            let mut p = pos.clone();

            p.z *= (iw as f32);
            p.w *= (ih as f32);
            p.x += p.z * x as f32;
            p.y += p.w * y as f32;
            let str = format!("{}{}", name, n);
            // log(format!(
            //     "made tile tex {} at {} {} {} {}",
            //     str, p.x, p.y, p.z, p.w
            // ));
            dictionary.lock().insert(str, p);
        }
    }
    //dictionary.lock().insert(name, pos);
}
pub fn load_tex_from_buffer(str: &String, buffer: &Vec<u8>) {
    // println!("ol testure {} is {}", str, buffer.len());
    match image::load_from_memory(buffer.as_slice()) {
        Ok(img) => sort_image(str, img),
        Err(err) => {}
    }
}
pub fn load_tex(str: &String) {
    log(format!("apply texture {}", str));

    match load_img(str) {
        Ok(img) => sort_image(str, img),
        Err(err) => {
            // dictionary
            //     .lock()
            //     .insert(name, cgmath::Vector4::new(0., 0., 0., 0.));
        }
    }
}
fn sort_image(str: &String, img: DynamicImage) {
    let (name, is_tile) = get_name(str.clone());
    if is_tile {
        let dim = (img.width(), img.height());
        let pos = locate(img.into_rgba8());
        tile_locate(name, dim, pos);
    } else {
        let pos = locate(img.into_rgba8());
        println!("sort_image name {} pos {}", name, pos);
        dictionary.lock().insert(name, pos);
    }
}
fn get_name(str: String) -> (String, bool) {
    let smol = str.split("/").collect::<Vec<_>>();
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

pub fn load_tex_from_img(str: String, im: &Vec<gltf::image::Data>) {
    let pvec = im
        .iter()
        .flat_map(|d| d.pixels.as_slice().to_owned())
        .collect::<Vec<_>>();

    let (actual_name, bool) = get_name(str);
    log(format!("inject image {} from buffer", actual_name));

    let mut pos = Vec4::new(0., 0., 0., 0.);
    let image_buffer = match image::RgbaImage::from_raw(64, 64, pvec) {
        Some(o) => o,
        None => {
            error("Failed to load texture from mesh".to_string());
            dictionary.lock().insert(actual_name, pos);
            return;
        }
    };

    // let pp = image::ImageBuffer::from_raw(128, 128, pvec.as_slice());

    //let pvec = im.iter().flat_map(|d| d.into()).collect::<Vec<_>>();

    // let pixels = pvec.as_slice();
    // println!("pixels length {}", (pixels.len() as f32).sqrt());

    // for i in pixels {
    //     print!("{},", i);
    // }

    //let v=im.as_chunks().;

    //let o = im.into_iter().zip(); //.map(|d| d.pixels);

    //let k = o.into_iter().map(|d| d.to_owned().as_slice()).collect();
    // let u = im
    //     .iter()
    //     .flat_map(|d| d.pixels)
    //     .collect::<[u8]>()
    //     .try_into()
    //     .unwrap();
    //let u = im.iter().map(|d| d.pixels.as_slice()).collect::<[u8]>();
    //let u = im.iter().map(|d| d.pixels.as_slice()).;

    // image::save_buffer_with_format(
    //     "assets/pic.png",
    //     &image_buffer,
    //     64,
    //     64,
    //     image::ColorType::Rgba8,
    //     image::ImageFormat::Png,
    // );

    //image::guess_format(buffer)

    /*
    match image::load_from_memory(&po) {
        Ok(i) => {
            println!("image is {} {}", i.width(), i.height());
            pos = locate(i.into_rgba8());
        }
        Err(er) => {
            error(format!("Cannot load in texture for mesh {} :: {}", str, er));
        }
    }*/

    pos = locate(image_buffer);

    dictionary.lock().insert(actual_name, pos);
}

pub fn get_tex(str: &String) -> Vec4 {
    match dictionary.lock().get(str) {
        Some(v) => v.clone(),
        None => Vec4::new(0., 0., 0., 0.),
    }
}

pub fn stich(master_img: &mut RgbaImage, source: RgbaImage, x: u32, y: u32) {
    image::imageops::overlay(master_img, &source, x, y);
}
pub fn write_tex(device: &wgpu::Device, queue: &Queue, texture: &Texture, img: &RgbaImage) {
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
    log("make master texture".to_string());
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

fn log(str: String) {
    crate::log::log(format!("🎨texture::{}", str));
}
fn error(str: String) {
    crate::log::error(format!("‼︎ERROR::🎨texture::{}", str));
}

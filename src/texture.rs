use std::{collections::HashMap, sync::Arc};

use image::{DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage};
use lazy_static::lazy_static;
use once_cell::sync::OnceCell;
use parking_lot::Mutex;
use wgpu::{Queue, Sampler, Texture, TextureView};

lazy_static! {
    //static ref controls: Arc<Mutex<bool>> = Arc::new(Mutex::new(false));
    pub static ref master: Arc<Mutex<OnceCell<RgbaImage>>> = Arc::new(Mutex::new(OnceCell::new()));
    pub static ref atlas_pos:Mutex<cgmath::Vector4<u32>>= Mutex::new(cgmath::Vector4::new(0,0,0,0));
    pub static ref atlas_dim:Mutex<cgmath::Vector2<u32>>= Mutex::new(cgmath::Vector2::new(0,0));
    pub static ref dictionary:Mutex<HashMap<String,cgmath::Vector4<f32>>> =Mutex::new(HashMap::new());
}

pub fn init() {
    let mut img: RgbaImage = ImageBuffer::new(512, 512);
    let mut d = atlas_dim.lock();
    d.x = 512;
    d.y = 512;
    let rgba = master.lock().get_or_init(|| img);
}
pub fn finalize(device: &wgpu::Device, queue: &Queue) -> (TextureView, Sampler) {
    make_tex(device, queue, master.lock().get().unwrap())
}
/**locate a position in the  master texture atlas, return a v4 of the tex coord x y offset and the scaleX scaleY to multiply the uv by to get the intended texture */
pub fn locate(source: RgbaImage) -> cgmath::Vector4<f32> {
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
    cgmath::Vector4::new(
        cpos.x as f32 / adim.x as f32,
        cpos.y as f32 / adim.y as f32,
        w as f32 / adim.x as f32,
        h as f32 / adim.y as f32,
    )
}

fn _load_img(str: String) -> DynamicImage {
    let text = format!("assets/{}.png", str);
    //Path::new(".").join("entities");
    let img = image::open(text).unwrap();
    // The dimensions method returns the images width and height.
    //println!("dimensions height {:?}", img.height());

    // The color method returns the image's `ColorType`.
    //println!("{:?}", img.color());
    img
}
pub fn load_tex(str: String) {
    log(format!("apply texture {}", str));
    let img = _load_img(str.clone());
    let pos = locate(img.into_rgba8());
    dictionary.lock().insert(str, pos);
}

pub fn load_tex_from_img(str: String, im: &Vec<gltf::image::Data>) {
    let pvec = im
        .iter()
        .flat_map(|d| d.pixels.as_slice().to_owned())
        .collect::<Vec<_>>();

    let mut pos = cgmath::Vector4::new(0., 0., 0., 0.);
    let image_buffer = match image::RgbaImage::from_raw(64, 64, pvec) {
        Some(o) => o,
        None => {
            error("Failed to load texture from mesh".to_string());
            dictionary.lock().insert(str, pos);
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
    image::save_buffer_with_format(
        "assets/pic.png",
        &image_buffer,
        64,
        64,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    );

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

    dictionary.lock().insert(str, pos);
}

pub fn get_tex(str: String) -> cgmath::Vector4<f32> {
    match dictionary.lock().get(&str) {
        Some(v) => v.clone(),
        None => cgmath::Vector4::new(0., 0., 0., 0.),
    }
}

pub fn stich(master_img: &mut RgbaImage, source: RgbaImage, x: u32, y: u32) {
    image::imageops::overlay(master_img, &source, x, y);
}
pub fn make_tex(device: &wgpu::Device, queue: &Queue, img: &RgbaImage) -> (TextureView, Sampler) {
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
    (diffuse_texture_view, diffuse_sampler)
}

fn log(str: String) {
    crate::log::log(format!("🎨texture::{}", str));
}
fn error(str: String) {
    crate::log::log(format!("‼︎ERROR::🎨texture::{}", str));
}

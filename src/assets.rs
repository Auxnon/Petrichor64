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
    if (apos.x != 0 && apos.y != 0) {
        //while (!found && apos.y < m_ref.height()) {

        if (apos.x + w) <= adim.x {
            found = true;
            apos.x += w;
        } else {
            apos.x = 0;
            apos.y += apos.w;
            if (apos.x + w) < adim.x && (apos.y + h) < adim.y {
                found = true;
            }
        }
        //if()
        //}
    } else {
        found = true;
        apos.x += w;
    }
    if found && apos.w < h {
        apos.w = h;
    }

    assert!(found, "Texture atlas couldnt find an empty spot?");
    println!("found position {} {}", cpos.x, cpos.y);
    stich(m_ref, source, cpos.x, cpos.y);
    println!("applied texture to atlas");
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
    println!("dimensions height {:?}", img.height());

    // The color method returns the image's `ColorType`.
    println!("{:?}", img.color());
    img
}
pub fn load_tex(str: String) {
    println!("apply texture {}", str);
    let img = _load_img(str.clone());
    let pos = locate(img.into_rgba8());
    dictionary.lock().insert(str, pos);
}

pub fn get_tex(str: String) -> cgmath::Vector4<f32> {
    match dictionary.lock().get(&str) {
        Some(v) => v.clone(),
        None => cgmath::Vector4::new(0., 0., 0., 0.),
    }
}

pub fn stich(master_img: &mut RgbaImage, source: RgbaImage, x: u32, y: u32) {
    println!("go");
    image::imageops::overlay(master_img, &source, x, y);
}
pub fn make_tex(device: &wgpu::Device, queue: &Queue, img: &RgbaImage) -> (TextureView, Sampler) {
    println!("make master texture");
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

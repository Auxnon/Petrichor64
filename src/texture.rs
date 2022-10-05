use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{template::AssetTemplate, world::World};
use glam::{vec4, UVec2, UVec4, Vec4};
use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect, draw_filled_rect_mut};
use wgpu::{Queue, Sampler, Texture, TextureView};

#[cfg(target_os = "windows")]
const SLASH: char = '\\';
#[cfg(not(target_os = "windows"))]
const SLASH: char = '/';

pub struct TexManager {
    pub MASTER: RgbaImage,
    /** Last position of a locatated section for a texture, x y, the last   */
    pub ATLAS_POS: UVec4,
    /** Current dimensions of the atlas image, likely stays the same in most cases */
    pub ATLAS_DIM: UVec2,
    /** Our wonderful string to uv coordinate map, give us a texture name and we'll give you a position on the atlas of that texture! */
    pub DICTIONARY: HashMap<String, Vec4>,
    // /** a really basic UUID, just incrementing from 0..n is fine internally, can we even go higher then 4294967295 (2^32 -1 is u32 max)?*/
    // pub COUNTER: u32,
    // /** map our texture strings to their integer value for fast lookup and 3d grid insertion*/
    // pub INT_DICTIONARY: HashMap<String, u32>,
    // pub INT_MAP: FxHashMap<u32, Vec4>,
    pub ANIMATIONS: HashMap<String, (Vec<Vec4>, u32)>,
}
impl TexManager {
    pub fn new() -> TexManager {
        TexManager {
            MASTER: ImageBuffer::new(1024, 1024),
            ATLAS_POS: UVec4::new(0, 0, 0, 0),
            ATLAS_DIM: UVec2::new(1024, 1024),
            DICTIONARY: HashMap::new(),
            // COUNTER: 2,
            // INT_DICTIONARY: HashMap::new(),
            // INT_MAP: FxHashMap::default(),
            ANIMATIONS: HashMap::default(),
        }
    }
    pub fn reset(&mut self) {
        let img: RgbaImage = ImageBuffer::new(1024, 1024);
        self.ATLAS_DIM.x = 1024;
        self.ATLAS_DIM.y = 1024;
        self.ATLAS_POS.x = 0;
        self.ATLAS_POS.y = 0;
        self.ATLAS_POS.z = 0;
        self.ATLAS_POS.w = 0;
        self.DICTIONARY.clear();
        image::imageops::replace(&mut self.MASTER, &img, 0, 0);
    }

    pub fn save_atlas(&mut self) {
        let dim = self.MASTER.dimensions();
        match image::save_buffer_with_format(
            "atlas.png",
            &self.MASTER,
            1024,
            1024,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        ) {
            Ok(_) => lg!("saved atlas {}x{}", dim.0, dim.1),
            Err(err) => lg!("failed to save atlas:{}", err),
        }
    }

    pub fn finalize(
        &self,
        device: &wgpu::Device,
        queue: &Queue,
    ) -> (TextureView, Sampler, Texture) {
        for (k, v) in self.DICTIONARY.iter() {
            println!("tex>>{}>>{}", k, v);
        }

        make_tex(device, queue, &self.MASTER)
    }

    pub fn refinalize(&self, queue: &Queue, texture: &Texture) {
        for (k, v) in self.DICTIONARY.iter() {
            println!("tex>>{}>>{}", k, v);
        }
        write_tex(queue, texture, &self.MASTER);
    }

    /**locate a position in the  master texture atlas, return a v4 of the tex coord x y offset and the scaleX scaleY to multiply the uv by to get the intended texture */
    pub fn locate(&mut self, source: RgbaImage) -> Vec4 {
        assert!(
            source.width() < self.MASTER.width() && source.height() < self.MASTER.height(),
            "Texture atlas isnt big enough for this image :("
        );
        let mut found = false;
        let mut cpos = self.ATLAS_POS.clone();
        let adim = self.ATLAS_DIM;
        let w = source.width();
        let h = source.height();

        if self.ATLAS_POS.x + w <= adim.x && self.ATLAS_POS.y + h <= adim.y {
            found = true;
            self.ATLAS_POS.x += w;
        } else {
            if self.ATLAS_POS.x + w > adim.x {
                self.ATLAS_POS.x = w;
                self.ATLAS_POS.y += self.ATLAS_POS.w;
                cpos.x = 0;
                cpos.y = self.ATLAS_POS.y;
                found = true;
            } else if self.ATLAS_POS.y + h < adim.y {
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
        if found && self.ATLAS_POS.w < h {
            self.ATLAS_POS.w = h;
        }

        assert!(found, "Texture atlas couldnt find an empty spot?");
        lg!("found position {} {}", cpos.x, cpos.y);
        stich(&mut self.MASTER, source, cpos.x, cpos.y);
        Vec4::new(
            cpos.x as f32 / adim.x as f32,
            cpos.y as f32 / adim.y as f32,
            w as f32 / adim.x as f32,
            h as f32 / adim.y as f32,
        )
    }

    fn tile_locate(
        &mut self,
        world: &mut World,
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
                let index_key = world.index_texture(key.clone(), p);

                // log(format!(
                //     "made tile tex {} at {} {} {} {}",
                //     key, p.x, p.y, p.z, p.w
                // ));
                self.DICTIONARY.insert(key, p);
                if can_rename {
                    match rename {
                        Some(ref r) => {
                            match r.get(&n) {
                                Some(name) => {
                                    self.DICTIONARY.insert(name.clone(), p);
                                    if index_key > 0 {
                                        world.index_texture_alias(name.clone(), index_key);
                                    }
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

    fn sort_image(
        &mut self,
        world: &mut World,
        str: &String,
        img: DynamicImage,
        template: Option<&AssetTemplate>,
        from_unpack: bool,
    ) {
        let (name, mut is_tile) = get_name(str.clone(), from_unpack);
        let mut rename = None;
        let mut tile_dim = if is_tile > 0 { is_tile } else { 16u32 };
        match template {
            Some(t) => {
                if t.tiles.len() > 0 || t.size > 0 {
                    is_tile = 16;
                }
                rename = Some(t.tiles.clone());
                tile_dim = t.size;
            }
            _ => {}
        }
        if is_tile > 0 {
            let dim = (img.width(), img.height());
            let pos = self.locate(img.into_rgba8());
            self.tile_locate(world, name, dim, pos, tile_dim, rename);
        } else {
            let pos = self.locate(img.into_rgba8());
            // lg!("sort_image name {} pos {}", name, pos);
            self.DICTIONARY.insert(name.clone(), pos);
            world.index_texture(name, pos);
        }
    }
    pub fn load_tex_from_buffer(
        &mut self,
        world: &mut World,
        str: &String,
        buffer: &Vec<u8>,
        template: Option<&AssetTemplate>,
    ) {
        // println!("🟢load_tex_from_buffer{} is {}", str, buffer.len());
        match image::load_from_memory(buffer.as_slice()) {
            Ok(img) => self.sort_image(world, str, img, template, true),
            Err(err) => err!("failed to load texture {}", err),
        }
    }

    pub fn load_tex(&mut self, world: &mut World, str: &String, template: Option<&AssetTemplate>) {
        // println!("🟢load_tex{}", str);
        lg!("apply texture {}", str);

        match load_img_nopath(str) {
            Ok(img) => self.sort_image(world, str, img, template, false),
            Err(err) => {
                err!("failed to load texture {}", err);
                // dictionary
                //     .lock()
                //     .insert(name, cgmath::Vector4::new(0., 0., 0., 0.));
            }
        }
    }

    pub fn load_tex_from_img(
        &mut self,
        short_name: String,
        path: String,
        im: &Vec<gltf::image::Data>,
    ) -> Vec4 {
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
                self.DICTIONARY.insert(short_name, pos);
                return vec4(1., 1., 0., 0.);
            }
        };

        pos = self.locate(image_buffer);

        // println!("🔥inject image {} from buffe {}", short_name, pos);

        self.DICTIONARY.insert(short_name.clone(), pos);
        pos
        // index_texture(short_name, pos);
    }

    /** return texture uv coordinates from a given texture name */
    pub fn get_tex(&self, str: &String) -> Vec4 {
        match self.DICTIONARY.get(str) {
            Some(v) => v.clone(),
            None => Vec4::new(0., 0., 0., 0.),
        }
    }
    pub fn get_tex_or_not(&self, str: &String) -> Option<Vec4> {
        match self.DICTIONARY.get(str) {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }

    pub fn _list_keys(&self) -> String {
        self.DICTIONARY
            .keys()
            .map(|k| k.clone())
            .collect::<Vec<String>>()
            .join(",")
    }

    pub fn set_anims(&mut self, name: &String, frames: Vec<Vec4>, animation_speed: u32) {
        println!("set anims {} {:?}", name, frames);
        self.ANIMATIONS
            .insert(name.clone(), (frames, animation_speed));
    }
}

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

pub fn render_sampler(device: &wgpu::Device, size: (u32, u32)) -> (TextureView, Sampler, Texture) {
    let img: RgbaImage = ImageBuffer::new(size.0, size.1);
    make_render_tex(device, &img)
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

fn get_name(str: String, from_unpack: bool) -> (String, u32) {
    if from_unpack {
        //TODO we can assume hopefully that no file extension was provided in an unpack call of this func, this should be fixed but it's complicated
        let bits = str.split(".").collect::<Vec<_>>();
        match bits.get(0) {
            Some(o) => {
                let b = bits.get(bits.len() - 1).unwrap();
                if bits.len() > 1 && b.starts_with("tile") {
                    return (o.to_string(), if b.ends_with("32") { 32 } else { 16 });
                }
                (o.to_string(), 0)
            }
            None => (str, 0),
        }
    } else {
        let smol = str.split(SLASH).collect::<Vec<_>>();
        let actual = smol.last().unwrap();
        let bits = actual.split(".").collect::<Vec<_>>();
        match bits.get(0) {
            Some(o) => {
                let b = bits.get(1).unwrap();
                if bits.len() > 2 && b.starts_with("tile") {
                    return (o.to_string(), if b.ends_with("32") { 32 } else { 16 });
                }
                (o.to_string(), 0)
            }
            None => (str, 0),
        }
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
    // lg!("make master texture");
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
    // lg!("make master texture");
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

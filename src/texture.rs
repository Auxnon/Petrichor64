use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use crate::{
    log::{LogType, Loggy},
    template::AssetTemplate,
    world::World,
};
use glam::{vec4, UVec2, UVec4, Vec4};
use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_rect, draw_filled_rect_mut};
use itertools::Itertools;
use rustc_hash::FxHashMap;
use wgpu::{Queue, Sampler, Texture, TextureView};

#[cfg(target_os = "windows")]
const SLASH: char = '\\';
#[cfg(not(target_os = "windows"))]
const SLASH: char = '/';

const MAX_WIDTH: u32 = 2048;
const MAX_HEIGHT: u32 = 2048;

pub struct TexTuple {
    pub view: TextureView,
    pub sampler: Sampler,
    pub texture: Texture,
}
impl TexTuple {
    pub fn new(view: TextureView, sampler: Sampler, texture: Texture) -> TexTuple {
        TexTuple {
            view,
            sampler,
            texture,
        }
    }
}

pub struct TexManager {
    pub atlas: RgbaImage,
    /** Last position of a locatated section for a texture, x y, the last   */
    pub atlas_pos: UVec4,
    /** Current dimensions of the atlas image, likely stays the same in most cases */
    pub atlas_dim: UVec2,
    /** Our wonderful string to uv coordinate map, give us a texture name and we'll give you a position on the atlas of that texture! */
    pub dictionary: HashMap<String, Vec4>,
    pub animations: HashMap<String, Anim>,
    pub bundle_lookup: FxHashMap<u8, Vec<(String, glam::Vec4)>>,
}
pub struct Anim {
    pub frames: Vec<Vec4>,
    pub speed: u32,
    pub once: bool,
}

impl Anim {
    pub fn empty() -> Anim {
        Anim {
            frames: vec![],
            speed: 16,
            once: false,
        }
    }
}
impl Clone for Anim {
    fn clone(&self) -> Anim {
        Anim {
            frames: self.frames.clone(),
            speed: self.speed,
            once: self.once,
        }
    }
}

impl TexManager {
    pub fn new() -> TexManager {
        TexManager {
            atlas: ImageBuffer::new(MAX_WIDTH, MAX_HEIGHT),
            atlas_pos: UVec4::new(0, 0, 0, 0),
            atlas_dim: UVec2::new(MAX_WIDTH, MAX_HEIGHT),
            dictionary: HashMap::new(),
            animations: HashMap::default(),
            bundle_lookup: FxHashMap::default(),
        }
    }
    pub fn reset(&mut self) {
        let img: RgbaImage = ImageBuffer::new(MAX_WIDTH, MAX_HEIGHT);
        self.atlas_dim.x = MAX_WIDTH;
        self.atlas_dim.y = MAX_HEIGHT;
        self.atlas_pos.x = 0;
        self.atlas_pos.y = 0;
        self.atlas_pos.z = 0;
        self.atlas_pos.w = 0;
        self.dictionary.clear();
        image::imageops::replace(&mut self.atlas, &img, 0, 0);
    }

    pub fn save_atlas(&mut self, loggy: &mut Loggy) {
        let dim = self.atlas.dimensions();
        match image::save_buffer_with_format(
            "atlas.png",
            &self.atlas,
            MAX_WIDTH,
            MAX_HEIGHT,
            image::ColorType::Rgba8,
            image::ImageFormat::Png,
        ) {
            Ok(_) => loggy.log(
                LogType::Texture,
                &format!("saved atlas {}x{}", dim.0, dim.1),
            ),
            Err(err) => loggy.log(
                LogType::TextureError,
                &format!("failed to save atlas:{}", err),
            ),
        }
    }

    pub fn finalize(&self, device: &wgpu::Device, queue: &Queue) -> TexTuple {
        make_tex(device, queue, &self.atlas)
    }

    pub fn refinalize(&self, queue: &Queue, texture: &Texture) {
        // for (k, v) in self.dictionary.iter() {
        //     println!("tex>>{}>>{}", k, v);
        // }
        write_tex(queue, texture, &self.atlas);
    }

    /**locate a position in the  master texture atlas, return a v4 of the tex coord x y offset and the scaleX scaleY to multiply the uv by to get the intended texture */
    pub fn locate(&mut self, source: RgbaImage) -> Vec4 {
        assert!(
            source.width() < self.atlas.width() && source.height() < self.atlas.height(),
            "Texture atlas isnt big enough for this image :("
        );
        let mut found = false;
        let mut cpos = self.atlas_pos.clone();
        let adim = self.atlas_dim;
        let w = source.width();
        let h = source.height();

        if self.atlas_pos.x + w <= adim.x && self.atlas_pos.y + h <= adim.y {
            found = true;
            self.atlas_pos.x += w;
        } else {
            if self.atlas_pos.x + w > adim.x {
                self.atlas_pos.x = w;
                self.atlas_pos.y += self.atlas_pos.w;
                cpos.x = 0;
                cpos.y = self.atlas_pos.y;
                found = true;
            } else if self.atlas_pos.y + h < adim.y {
                panic!("Texture atlas couldnt find an empty spot?");
            }
        }
        if found && self.atlas_pos.w < h {
            self.atlas_pos.w = h;
        }

        assert!(found, "Texture atlas couldnt find an empty spot?");
        // lg!("found position ({},{}) and ({},{}) ", cpos.x, cpos.y, w, h);
        stich(&mut self.atlas, source, cpos.x, cpos.y);
        Vec4::new(
            cpos.x as f32 / adim.x as f32,
            cpos.y as f32 / adim.y as f32,
            w as f32 / adim.x as f32,
            h as f32 / adim.y as f32,
        )
    }

    /** Locates all tiles within a single provided image, indexing by provided tile dimension, 16x16 by default.
     * Each tile iterates starting at 0. An image named Example with 3 tiles will be accessed via Example0,Example1,Example2.
     * Returns the tile referenced at index 0 so accessing this texture named Example is still possible without calling Example0  */
    fn tile_locate(
        &mut self,
        world: &mut World,
        bundle_id: u8,
        name: &String,
        dim: (u32, u32),
        pos: Vec4,
        mut tile_dim: u32,
        rename: Option<HashMap<u32, String>>,
    ) -> Vec4 {
        if tile_dim == 0 {
            tile_dim = 16;
        }
        let dw = dim.0 / tile_dim;
        let dh = dim.1 / tile_dim;
        let iw = 1. / dw as f32;
        let ih = 1. / dh as f32;

        let mut first_complete = false;
        let mut first = vec4(0., 0., 1., 1.);
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

                if !first_complete {
                    first = p.clone();
                    first_complete = true;
                }
                let index_key = world.index_texture(bundle_id, key.clone(), p);

                // log(format!(
                //     "made tile tex {} at {} {} {} {}",
                //     key, p.x, p.y, p.z, p.w
                // ));
                self.dictionary.insert(key, p);
                if can_rename {
                    match rename {
                        Some(ref r) => {
                            match r.get(&n) {
                                Some(name) => {
                                    self.dictionary.insert(name.clone(), p);
                                    if index_key > 0 {
                                        world.index_texture_alias(
                                            bundle_id,
                                            name.clone(),
                                            index_key,
                                        );
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
        first
        //dictionary.lock().insert(name, pos);
    }

    fn sort_image(
        &mut self,
        world: &mut World,
        name_like: &str,
        img: RgbaImage,
        bundle_id: u8,
        template: Option<&AssetTemplate>,
        from_unpack: bool,
        loggy: &mut Loggy,
    ) {
        let (name, mut is_tile) = get_name(name_like, from_unpack);
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
        let pos = if is_tile > 0 {
            let dim = (img.width(), img.height());
            let pos = self.locate(img);
            self.tile_locate(world, bundle_id, &name, dim, pos, tile_dim, rename)
        } else {
            self.locate(img)
        };
        // TODO
        match self.bundle_lookup.get_mut(&bundle_id) {
            Some(b) => {
                b.push((name.clone(), pos));
            }
            None => {
                let b = vec![(name.clone(), pos)];
                self.bundle_lookup.insert(bundle_id, b);
            }
        }

        loggy.log(
            LogType::Texture,
            &format!("sort_image name {} pos {} tile_size {}", name, pos, is_tile),
        );
        self.dictionary.insert(name.clone(), pos);
        world.index_texture(bundle_id, name, pos);
    }

    /** Load image from a buffer, likely from a game bin, zip or game image zip */
    pub fn load_tex_from_buffer(
        &mut self,
        world: &mut World,
        name_like: &str,
        buffer: &Vec<u8>,
        bundle_id: u8,
        template: Option<&AssetTemplate>,
        loggy: &mut Loggy,
    ) {
        // println!("🟢load_tex_from_buffer{} is {}", str, buffer.len());
        match image::load_from_memory(buffer.as_slice()) {
            Ok(img) => self.sort_image(
                world,
                name_like,
                img.into_rgba8(),
                bundle_id,
                template,
                true,
                loggy,
            ),
            Err(err) => {
                loggy.log(
                    LogType::TextureError,
                    &format!("failed to load texture {}", err),
                );
            }
        }
    }

    /** Load image directly from file system and apply to atlas */
    pub fn load_tex(
        &mut self,
        world: &mut World,
        name_like: &String,
        bundle_id: u8,
        template: Option<&AssetTemplate>,
        loggy: &mut Loggy,
    ) {
        loggy.log(LogType::Texture, &format!("apply texture {}", name_like));

        match load_img_nopath(name_like, loggy) {
            Ok(img) => self.sort_image(
                world,
                name_like,
                img.into_rgba8(),
                bundle_id,
                template,
                false,
                loggy,
            ),
            Err(err) => {
                loggy.log(
                    LogType::TextureError,
                    &format!("failed to load texture {}", err),
                );
            }
        }
    }

    /** Loads images from a gltf into dictionary and returns array of uv rects for each resource */
    pub fn load_tex_from_data(
        &mut self,
        short_name: String,
        path: String,
        images: &Vec<gltf::image::Data>,
        loggy: &mut Loggy,
    ) -> Vec<Vec4> {
        images
            .iter()
            .map(|imm| {
                // println!("w {} h{}", imm.width, imm.height);
                let im = imm.pixels.as_slice().to_owned();

                let pos = match image::RgbaImage::from_raw(imm.width, imm.height, im) {
                    Some(o) => self.locate(o),
                    None => {
                        loggy.log(LogType::TextureError, &"Failed to load texture from mesh");
                        vec4(0., 0., 1., 1.)
                    }
                };

                self.dictionary.insert(short_name.clone(), pos);
                pos
            })
            .collect::<Vec<Vec4>>()
    }

    pub fn overwrite_texture(
        &mut self,
        name: &str,
        source: RgbaImage,
        world: &mut World,
        bundle_id: u8,
        loggy: &mut Loggy,
    ) {
        match self.get_tex_or_not(name) {
            Some(pos) => {
                // overlay(master_img, &source, x as i64, y as i64);
                image::imageops::replace(
                    &mut self.atlas,
                    &source,
                    (pos.x * self.atlas_dim.x as f32) as i64,
                    (pos.y * self.atlas_dim.y as f32) as i64,
                );
                // image::imageops::overlay(master_img, &source, x as i64, y as i64);
            }
            None => {
                // println!("brand new texture {}", name);
                self.sort_image(world, name, source, bundle_id, None, true, loggy)
                // self.load_tex_from_buffer(world, name, &source.to_vec(), bundle_id, None, loggy)
            }
        }
    }

    /** return texture uv coordinates from a given texture name */
    pub fn get_tex(&self, str: &str) -> Vec4 {
        match self.dictionary.get(str) {
            Some(v) => v.clone(),
            None => Vec4::new(0., 0., 1., 1.),
        }
    }
    pub fn get_tex_or_not(&self, str: &str) -> Option<Vec4> {
        match self.dictionary.get(str) {
            Some(v) => Some(v.clone()),
            None => None,
        }
    }

    /** return the actual image buffer data from a given texture name */
    pub fn get_img(&self, str: &String) -> (u32, u32, RgbaImage) {
        let im = match self.dictionary.get(str) {
            Some(v) => image::imageops::crop_imm(
                &self.atlas,
                (v.x * self.atlas_dim.x as f32) as u32,
                (v.y * self.atlas_dim.y as f32) as u32,
                (v.z * self.atlas_dim.x as f32) as u32,
                (v.w * self.atlas_dim.y as f32) as u32,
            )
            .to_image(),
            None => self.atlas.clone(),
        };

        (im.width(), im.height(), im)
    }

    fn get_img_from_pos(&self, pos: &Vec4) -> RgbaImage {
        image::imageops::crop_imm(
            &self.atlas,
            (pos.x * self.atlas_dim.x as f32) as u32,
            (pos.y * self.atlas_dim.y as f32) as u32,
            (pos.z * self.atlas_dim.x as f32) as u32,
            (pos.w * self.atlas_dim.y as f32) as u32,
        )
        .to_image()
    }

    pub fn _list_keys(&self) -> String {
        self.dictionary
            .keys()
            .map(|k| k.clone())
            .collect::<Vec<String>>()
            .join(",")
    }

    pub fn set_anims(
        &mut self,
        name: &String,
        frames: Vec<Vec4>,
        animation_speed: u32,
        once: bool,
    ) {
        println!("set anims {} {:?}", name, frames);
        self.animations.insert(
            name.clone(),
            Anim {
                frames,
                speed: animation_speed,
                once,
            },
        );
    }
    pub fn remove_bundle_content(&mut self, bundle_id: u8) {
        self.bundle_lookup.remove(&bundle_id);
    }

    pub fn rebuild_atlas(&mut self, world: &mut World, loggy: &mut Loggy) {
        self.atlas = image::RgbaImage::new(self.atlas_dim.x, self.atlas_dim.y);
        self.dictionary = HashMap::new();
        let hash = self.bundle_lookup.drain().collect_vec();
        let images = hash
            .iter()
            .map(|(k, v)| {
                let vv = v
                    .iter()
                    .map(|(s, pos)| {
                        let im = self.get_img_from_pos(pos);
                        (s, im)
                    })
                    .collect_vec();
                (k, vv)
            })
            .collect_vec();
        self.reset();
        for (bundle_id, v) in images {
            for (name, img) in v {
                self.sort_image(world, &name, img, *bundle_id, None, true, loggy);
            }
        }
        //
    }
}

pub fn simple_square(size: u32, path: PathBuf, loggy: &mut Loggy) {
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
        Err(err) => loggy.log(
            LogType::TextureError,
            &format!("could not save example image: {}", err),
        ),
        _ => {}
    }
}

pub fn save_audio_buffer(buffer: &Vec<u8>, loggy: &mut Loggy) {
    // println!("🟣buffer {} {}", buffer.len(), 512);
    let w = buffer.len() as u32;
    let h = 512;
    let img: RgbaImage = ImageBuffer::new(w, h);

    let yellow = image::Rgba::from([255, 255, 0, 255]);
    let black = image::Rgba::from([0, 0, 0, 255]);
    let rect = imageproc::rect::Rect::at(0, 0).of_size(w, h);
    let mut new_img = draw_filled_rect(&img, rect, yellow);
    for (i, c) in buffer.iter().enumerate() {
        new_img.put_pixel(i as u32, *c as u32, black);
    }

    match image::save_buffer_with_format(
        "sound.png",
        &new_img,
        w,
        h,
        image::ColorType::Rgba8,
        image::ImageFormat::Png,
    ) {
        Err(err) => {
            loggy.log(
                LogType::TextureError,
                &format!("unable to save image: {}", err),
            );
        }
        _ => {}
    }
}

pub fn render_sampler(device: &wgpu::Device, size: (u32, u32)) -> (TextureView, Sampler, Texture) {
    let img: RgbaImage = ImageBuffer::new(size.0, size.1);
    make_render_tex(device, &img)
}

pub fn load_img(str: &str, loggy: &mut Loggy) -> Result<DynamicImage, image::ImageError> {
    let text = Path::new("assets").join(str).to_str().unwrap().to_string();
    //Path::new(".").join("entities");
    load_img_nopath(&text, loggy)
}

pub fn load_img_nopath(str: &str, loggy: &mut Loggy) -> Result<DynamicImage, image::ImageError> {
    loggy.log(LogType::Texture, &format!("loading image {}", str));

    let img = image::open(str);

    // The dimensions method returns the images width and height.
    //println!("dimensions height {:?}", img.height());

    img
}

pub fn load_img_from_buffer(buffer: &[u8]) -> Result<DynamicImage, image::ImageError> {
    let img = image::load_from_memory(buffer);
    img
}

fn get_name(str: &str, from_unpack: bool) -> (String, u32) {
    if from_unpack {
        //TODO we can assume hopefully that no file extension was provided in an unpack call of this func, this should be fixed but it's complicated
        let bits = str.split(".").collect::<Vec<_>>();
        match bits.get(0) {
            Some(o) => {
                if bits.len() > 1 {
                    let b = bits.get(bits.len() - 1).unwrap();
                    return (o.to_string(), check_tile_size(b));
                }
                (o.to_string(), 0)
            }
            None => (str.to_string(), 0),
        }
    } else {
        let smol = str.split(SLASH).collect::<Vec<_>>();
        let actual = smol.last().unwrap();
        let bits = actual.split(".").collect::<Vec<_>>();
        match bits.get(0) {
            Some(o) => {
                // println!("compare {} for {} ", bits.len(), actual);
                if bits.len() > 2 {
                    let b = bits.get(1).unwrap();
                    // println!("compare {} for {} ", bits.len(), actual);
                    return (o.to_string(), check_tile_size(b));
                }
                (o.to_string(), 0)
            }
            None => (str.to_string(), 0),
        }
    }
}

fn check_tile_size(str: &str) -> u32 {
    match if str.starts_with("tile") {
        // cuts string to just the number
        str[4..].parse::<u32>()
    } else {
        str.parse::<u32>()
    } {
        Ok(o) => o,
        _ => 16,
    }
}
/** Apply an image on to a main image such as the atlas */
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

pub fn make_tex(device: &wgpu::Device, queue: &Queue, img: &RgbaImage) -> TexTuple {
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
        view_formats: &[],
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
    TexTuple {
        view: diffuse_texture_view,
        sampler: diffuse_sampler,
        texture: tex,
    }
}

pub fn make_render_tex(device: &wgpu::Device, img: &RgbaImage) -> (TextureView, Sampler, Texture) {
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
        view_formats: &[],
    });

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

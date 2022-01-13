use rand::Rng;
use std::{collections::HashMap, path::Path, sync::Arc};

use image::{ImageBuffer, RgbaImage};
use once_cell::sync::OnceCell;
use wgpu::{Device, QuerySet, Queue, Sampler, Texture, TextureView};

use crate::model::{get_model, Model};

pub struct Gui {
    pub gui_pipeline: wgpu::RenderPipeline,
    pub gui_group: wgpu::BindGroup,
    pub model: Arc<OnceCell<Model>>,
    pub texture: Texture,
    pub img: RgbaImage,
    pub letters: RgbaImage,
    pub size: [u32; 2],
}
impl Gui {
    pub fn new(
        gui_pipeline: wgpu::RenderPipeline,
        gui_group: wgpu::BindGroup,
        texture: Texture,
        img: RgbaImage,
    ) -> Gui {
        let d = img.dimensions();

        let letters = crate::texture::load_img("3x3letters.png".to_string()).into_rgba8();

        Gui {
            gui_pipeline,
            gui_group,
            model: get_model(&"plane".to_string()),
            texture,
            img,
            letters,
            size: [d.0, d.1],
        }
    }

    pub fn render(&mut self, device: &Device, queue: &Queue) {
        let mut rng = rand::thread_rng();
        let mut x: u32 = (rng.gen_range(0..self.size[0]) / 4) * 4;
        let y: u32 = (rng.gen_range(0..self.size[1]) / 4) * 4;
        //let index_x = rng.gen_range(0..26) * 3;
        //let index_y = rng.gen_range(0..3) * 3;

        // let mut v = HashMap::new();
        // for i in 0..26 {
        //     v.insert(i + 10, i + 0 + 26);
        // }

        // v.insert(10, 0 + 26);
        // v.insert(10, 0 + 26);
        // v.insert(10, 0 + 26);
        // v.insert(10, 0 + 26);
        // v.insert(10, 0 + 26);
        // v.insert(10, 0 + 26);
        // v.insert(10, 0 + 26);
        // v.insert(10, 0 + 26);

        // v.insert(29, 19 + 26);
        // v.insert(14, 4 + 26);
        // v.insert(28, 18 + 26);
        //v.insert(index, element)

        let test = "Hello there 01234 ";

        for c in test.chars() {
            let res = c.to_digit(36);
            let ind = match res {
                Some(u) => {
                    //match v.get(&u) {
                    // Some(o) => *o,
                    // None => u,
                    if u < 10 {
                        u
                    } else {
                        u + 16
                    }
                }

                None => 3 * 26 + 25,
            };

            let index_x = (ind % 26);
            let index_y = (ind / 26);
            println!("c{} ind{} x {} y{}", c, ind, index_x, index_y);
            let sub = image::imageops::crop_imm(&self.letters, index_x * 3, index_y * 3, 3, 3);
            image::imageops::replace(&mut self.img, &sub, x, y);
            crate::texture::write_tex(device, queue, &self.texture, &self.img);
            x += 4;
        }
        println!("");
        // let rgb = image::Rgba([
        //     rng.gen_range(0..255) as u8,
        //     rng.gen_range(0..255) as u8,
        //     rng.gen_range(0..255) as u8,
        //     255u8,
        // ]);
        // image::imageops::
        // self.img.put_pixel(x, y, rgb);
        //image::imageops::overlay_bounds((3, 3), (3u32, 3u32), x, y);
        //image::imageops::overlay(&mut self.img, &self.letters, x, y);

        //let out = crate::texture::make_tex(device, queue, &self.img);
    }
}
pub fn init_image(
    device: &Device,
    queue: &Queue,
    aspect: f32,
) -> (TextureView, Sampler, Texture, RgbaImage) {
    let h = (256. / aspect).ceil() as u32;
    println!("aspect {}", h);
    let mut img: RgbaImage = ImageBuffer::new(256, h);
    img.put_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
    let out = crate::texture::make_tex(device, queue, &img);
    (out.0, out.1, out.2, img)
}
pub fn draw() {}

use rand::Rng;
use std::sync::Arc;
// use tracy::frame;

use crate::model::{get_model, Model};
use image::{ImageBuffer, RgbaImage};
use once_cell::sync::OnceCell;
use wgpu::{Device, Queue, Sampler, Texture, TextureView};

pub struct Gui {
    pub gui_pipeline: wgpu::RenderPipeline,
    pub gui_group: wgpu::BindGroup,
    pub model: Arc<OnceCell<Model>>,
    pub texture: Texture,
    text: String,
    pub main: RgbaImage,
    pub console: RgbaImage,

    time: f32,
    pub letters: RgbaImage,
    pub size: [u32; 2],
    dirty: bool,
    output: bool,
}

impl Gui {
    pub fn new(
        gui_pipeline: wgpu::RenderPipeline,
        gui_group: wgpu::BindGroup,
        texture: Texture,
        img: RgbaImage,
    ) -> Gui {
        let d = img.dimensions();

        let letters = match crate::texture::load_img(&"4x4unicode.png".to_string()) {
            Ok(img) => img.into_rgba8(),
            Err(_) => {
                #[cfg(not(windows))]
                macro_rules! sp {
                    () => {
                        "/"
                    };
                }

                #[cfg(windows)]
                macro_rules! sp {
                    () => {
                        r#"\"#
                    };
                }

                let d = include_bytes!(concat!("..", sp!(), "assets", sp!(), "4x4unicode.png"));
                crate::texture::load_img_from_buffer(d)
                    .unwrap()
                    .into_rgba8()
            }
        };

        // let raster: RgbaImage = ImageBuffer::new(1024, 1024);

        Gui {
            gui_pipeline,
            gui_group,
            model: get_model(&"plane".to_string()),
            texture,
            console: img.clone(),
            main: img,
            text: "".to_string(),
            letters,
            time: 0.,
            size: [d.0, d.1],
            dirty: true,
            output: false,
        }
    }

    //pub fn type()
    pub fn add_text(&mut self, str: String) {
        self.text = format!("{}\n{}", self.text, str);
        self.apply_text();
    }
    pub fn type_text(&mut self, str: String) {
        self.text = format!("{}\n{}", self.text, str);
    }
    pub fn add_img(&self, str: &String) {
        match crate::texture::load_img(str) {
            Ok(t) => {
                let mut im = t.to_rgba8();
                let w = im.width() / 16;
                let h = im.height() / 16;
                for x in 0..w {
                    for y in 0..h {
                        let n = x + y * w;
                        let digits: Vec<_> = n
                            .to_string()
                            .chars()
                            .map(|d| d.to_digit(10).unwrap())
                            .collect();
                        for (i, d) in digits.into_iter().enumerate() {
                            let sub = image::imageops::crop_imm(&self.letters, d * 4, 12, 4, 4);
                            // let imm: &ImageBuffer<Rgba<u8>, Vec<u8>> = sub.inner();
                            image::imageops::replace(
                                &mut im,
                                sub.inner(),
                                (x * 16 + i as u32 * 5).into(),
                                (y * 16).into(),
                            );
                        }

                        //image::imageops::overlay(&mut self.img, &sub, x, y);
                    }
                }

                image::save_buffer_with_format(
                    "tile.png",
                    &im,
                    im.width(),
                    im.height(),
                    image::ColorType::Rgba8,
                    image::ImageFormat::Png,
                );
            }
            Err(e) => {
                log(format!("{}", e));
            }
        }
    }
    pub fn apply_text(&mut self) {
        let im = RgbaImage::new(self.size[0], self.size[1]);
        // image::imageops::horizontal_gradient(
        //     &mut im,
        //     &image::Rgba([255, 255, 0, 255]),
        //     &image::Rgba([0, 0, 0, 0]),
        // );
        // self.img = RgbaImage::new(self.size[0], self.size[1]);
        image::imageops::replace(&mut self.console, &im, 0, 0);
        // struct col {};
        // impl image::imageops::colorops::ColorMap for col {
        //     type Color = image::Rgba<u8>;

        //     fn index_of(&self, color: &Self::Color) -> usize {
        //         // todo!()
        //         0
        //     }

        //     fn map_color(&self, color: &mut Self::Color) {
        //         //todo!()
        //         color.0[0] = 255;
        //         color.0[1] = 0;
        //     }
        // };
        // image::imageops::dither(&mut self.img, &col {});
        for (i, line) in self.text.lines().enumerate() {
            let y = i as i64 * 6;
            let mut x = 4;
            for c in line.chars() {
                let mut ind = c as u32;
                // let res = c.to_digit(36);
                // let ind = match res {
                //     Some(u) => {
                //         //match v.get(&u) {
                //         // Some(o) => *o,
                //         // None => u,
                //         if u < 10 {
                //             u
                //         } else {
                //             u + 16
                //         }
                //     }

                //     None => 3 * 26 + 25,
                // };
                if ind > 255 {
                    ind = 255;
                }
                let index_x = ind % 16;
                let index_y = ind / 16;
                //println!("c{} ind{} x {} y{}", c, ind, index_x, index_y);
                let sub = image::imageops::crop_imm(&self.letters, index_x * 4, index_y * 4, 4, 4);
                //sub.to_image().
                image::imageops::overlay(&mut self.console, sub.inner(), x, y);
                x += 5;
            }
        }

        self.console =
            image::imageops::huerotate(&mut self.console, rand::thread_rng().gen_range(0..360));
        self.dirty = true;
    }

    pub fn enable_output(&mut self) {
        self.text = crate::log::get(
            (self.size[0] / 5 - 2) as usize,
            (self.size[1] / 5 - 8) as usize,
        );
        self.output = true;
        self.apply_text();
    }
    pub fn disable_output(&mut self) {
        self.text = "".to_string();
        self.output = false;
        self.apply_text();
    }
    // pub fn toggle_output(&mut self) {
    //     if self.output {
    //         self.disable_output();
    //     } else {
    //         self.enable_output();
    //     }
    // }
    // fn draw_img();
    pub fn render(&mut self, queue: &Queue, time: f32) {
        //let mut rng = rand::thread_rng();
        // frame!("gui start");
        self.time = time;
        if self.output && crate::log::is_dirty() {
            self.text = crate::log::get(
                (self.size[0] / 5 - 2) as usize,
                (self.size[1] / 5 - 8) as usize,
            );
            self.apply_text();
            crate::log::clean();
        }
        if self.dirty {
            let raster = if self.output {
                &self.console
            } else {
                &self.main
            };
            crate::texture::write_tex(queue, &self.texture, raster);
            self.dirty = false;
        }

        if time % 0.2 == 0.0 {
            //log(format!("time {}", self.time));
            //self.img = image::imageops::huerotate(&mut self.img, (time * 360.) as i32);
            //crate::texture::write_tex(device, queue, &self.texture, &self.img);
        }

        // frame!("gui end");

        // let mut x: u32 = (rng.gen_range(0..self.size[0]) / 4) * 4;
        // let y: u32 = (rng.gen_range(0..self.size[1]) / 4) * 4;
        // let mut x: u32 = ((((self.time * 7.4).cos() * 128. + 128.) / 4.).ceil() * 4.) as u32;
        // let mut y: u32 = (((self.time.sin() * 128. + 128.) / 4.).ceil() * 4.) as u32;

        // let test = "01234567 Sup Thar 76543210";

        // for c in test.chars() {
        //     let res = c.to_digit(36);
        //     let ind = match res {
        //         Some(u) => {
        //             //match v.get(&u) {
        //             // Some(o) => *o,
        //             // None => u,
        //             if u < 10 {
        //                 u
        //             } else {
        //                 u + 16
        //             }
        //         }

        //         None => 3 * 26 + 25,
        //     };

        //     let index_x = (ind % 26);
        //     let index_y = (ind / 26);
        //     //println!("c{} ind{} x {} y{}", c, ind, index_x, index_y);
        //     let sub = image::imageops::crop_imm(&self.letters, index_x * 3, index_y * 3, 3, 3);
        //     //sub.to_image().
        //     image::imageops::replace(&mut self.img, &sub, x, y);
        //     crate::texture::write_tex(device, queue, &self.texture, &self.img);
        //     x += 4;
        // }
        // // println!("");
        // // let rgb = image::Rgba([
        // //     rng.gen_range(0..255) as u8,
        // //     rng.gen_range(0..255) as u8,
        // //     rng.gen_range(0..255) as u8,
        // //     255u8,
        // // ]);
        // // image::imageops::
        // // self.img.put_pixel(x, y, rgb);
        // //image::imageops::overlay_bounds((3, 3), (3u32, 3u32), x, y);
        // //image::imageops::overlay(&mut self.img, &self.letters, x, y);

        // //let out = crate::texture::make_tex(device, queue, &self.img);
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

fn log(str: String) {
    crate::log::log(format!("📺gui::{}", str));
}

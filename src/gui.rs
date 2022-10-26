use rand::Rng;
// use tracy::frame;

use crate::{command::NumCouple, texture::TexManager};
use image::{ImageBuffer, RgbaImage};
use imageproc::drawing::{draw_filled_rect, draw_filled_rect_mut};
use wgpu::{Device, Queue, Sampler, Texture, TextureView};

const LETTER_SIZE: u32 = 6;
const GUI_DIM: u32 = 320;

pub struct Gui {
    pub gui_pipeline: wgpu::RenderPipeline,
    pub sky_pipeline: wgpu::RenderPipeline,
    pub gui_group: wgpu::BindGroup,
    pub sky_group: wgpu::BindGroup,
    // pub model: Arc<OnceCell<Model>>,
    pub overlay_texture: Texture,
    pub sky_texture: Texture,
    text: String,
    /** main game rendered gui raster to stay in memory if toggled to console */
    pub main: RgbaImage,
    /** console raster with current output to stay in memory*/
    pub console: RgbaImage,
    /** skybox raster */
    pub sky: RgbaImage,

    time: f32,
    pub letters: RgbaImage,
    pub size: [u32; 2],
    // console_dity: bool,
    // main_dirty: bool,
    dirty: bool,
    dirty_sky: bool,
    target_sky: bool,
    output: bool,
}

impl Gui {
    pub fn new(
        gui_pipeline: wgpu::RenderPipeline,
        sky_pipeline: wgpu::RenderPipeline,
        gui_group: wgpu::BindGroup,
        overlay_texture: Texture,
        sky_group: wgpu::BindGroup,
        sky_texture: Texture,
        gui_img: RgbaImage,
    ) -> Gui {
        let d = gui_img.dimensions();

        let letters = match crate::texture::load_img(&"6x6unicode.png".to_string()) {
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

                let d = include_bytes!(concat!("..", sp!(), "assets", sp!(), "6x6unicode.png"));
                crate::texture::load_img_from_buffer(d)
                    .unwrap()
                    .into_rgba8()
            }
        };

        // let raster: RgbaImage = ImageBuffer::new(1024, 1024);

        Gui {
            gui_pipeline,
            sky_pipeline,
            gui_group,
            sky_group,
            // model: get_model(&"plane".to_string()),
            overlay_texture,
            sky_texture,
            console: gui_img.clone(),
            sky: gui_img.clone(),
            dirty_sky: false,
            target_sky: false,
            main: gui_img,
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
        self.apply_console_out_text();
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
                            let sub = image::imageops::crop_imm(
                                &self.letters,
                                d * LETTER_SIZE,
                                12,
                                LETTER_SIZE,
                                LETTER_SIZE,
                            );
                            // let imm: &ImageBuffer<Rgba<u8>, Vec<u8>> = sub.inner();
                            image::imageops::replace(
                                &mut im,
                                &mut sub.to_image(),
                                (x * 16 + i as u32 * (LETTER_SIZE + 1)).into(),
                                (y * 16).into(),
                            );
                        }

                        //image::imageops::overlay(&mut self.img, &sub, x, y);
                    }
                }

                // image::save_buffer_with_format(
                //     "tile.png",
                //     &im,
                //     im.width(),
                //     im.height(),
                //     image::ColorType::Rgba8,
                //     image::ImageFormat::Png,
                // );
            }
            Err(e) => {
                log(format!("{}", e));
            }
        }
    }
    pub fn direct_text(&mut self, txt: &String, onto_console: bool, x: i64, y: i64) {
        let targ = if onto_console {
            &mut self.console
        } else {
            if self.target_sky {
                self.dirty_sky = true;
                &mut self.sky
            } else {
                self.dirty = true;
                &mut self.main
            }
        };

        for (i, line) in txt.lines().enumerate() {
            let ly = y + i as i64 * (LETTER_SIZE as i64 + 2);
            let mut lx = x + (LETTER_SIZE) as i64;
            for c in line.chars() {
                let mut ind = c as u32;
                if ind > 255 {
                    ind = 255;
                }
                let index_x = ind % 16;
                let index_y = ind / 16;
                let sub = image::imageops::crop_imm(
                    &self.letters,
                    index_x * LETTER_SIZE,
                    index_y * LETTER_SIZE,
                    LETTER_SIZE,
                    LETTER_SIZE,
                );
                image::imageops::overlay(targ, &mut sub.to_image(), lx, ly);
                lx += (LETTER_SIZE + 1) as i64;
            }
        }

        // *targ = image::imageops::huerotate(targ, rand::thread_rng().gen_range(0..360));
        // self.dirty = true;
    }

    pub fn draw_image(
        &mut self,
        tex_manager: &mut TexManager,
        image: &String,
        onto_console: bool,
        x: i64,
        y: i64,
    ) {
        let targ = if onto_console {
            &mut self.console
        } else {
            if self.target_sky {
                self.dirty_sky = true;
                &mut self.sky
            } else {
                self.dirty = true;
                &mut self.main
            }
        };
        let source = tex_manager.get_tex(image);

        let w = tex_manager.MASTER.width();
        let h = tex_manager.MASTER.height();
        let sub = image::imageops::crop_imm(
            &mut tex_manager.MASTER,
            (source.x * w as f32) as u32,
            (source.y * h as f32) as u32,
            (source.z * w as f32) as u32,
            (source.w * h as f32) as u32,
        );
        image::imageops::overlay(targ, &mut sub.to_image(), x, y);

        // *targ = image::imageops::huerotate(targ, rand::thread_rng().gen_range(0..360));
        // self.dirty = true;
    }

    pub fn apply_console_out_text(&mut self) {
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

        const WHITE: image::Rgba<u8> = image::Rgba([255, 255, 0, 100]);

        for (i, line) in self.text.lines().enumerate() {
            let y = i as i64 * (LETTER_SIZE as i64 + 2);
            let mut x = (LETTER_SIZE) as i64;
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
                let sub = image::imageops::crop_imm(
                    &self.letters,
                    index_x * LETTER_SIZE,
                    index_y * LETTER_SIZE,
                    LETTER_SIZE,
                    LETTER_SIZE,
                );

                draw_filled_rect_mut(
                    &mut self.console,
                    imageproc::rect::Rect::at((x - 1) as i32, (y - 1) as i32)
                        .of_size(LETTER_SIZE + 2, LETTER_SIZE + 2 as u32),
                    WHITE,
                );
                //sub.to_image().
                image::imageops::overlay(&mut self.console, &mut sub.to_image(), x, y);
                x += (LETTER_SIZE + 1) as i64;
            }
        }

        self.console =
            image::imageops::huerotate(&mut self.console, rand::thread_rng().gen_range(0..360));
        self.dirty = true;
    }

    pub fn fill(&mut self, r: f32, g: f32, b: f32, a: f32) {
        let width = self.size[0];
        let height = self.size[1];

        draw_filled_rect_mut(
            self.get_targ(),
            imageproc::rect::Rect::at(0 as i32, 0 as i32).of_size(width as u32, height as u32),
            image::Rgba([
                (r * 255.) as u8,
                (g * 255.) as u8,
                (b * 255.) as u8,
                (a * 255.) as u8,
            ]),
        );
    }

    pub fn target_gui(&mut self) {
        self.target_sky = false;
    }
    pub fn target_sky(&mut self) {
        self.target_sky = true;
    }

    pub fn square(&mut self, x: NumCouple, y: NumCouple, w: NumCouple, h: NumCouple) {
        let width = self.size[0];
        let height = self.size[1];

        let xx = x.1.max(0.) * if x.0 { 1. } else { width as f32 };
        let yy = y.max(0.) * height as f32;
        let ww = (w.max(0.) * width as f32).max(1.);
        let hh = (h.max(0.) * height as f32).max(1.);
        // let mut im = RgbaImage::new(w, h);
        // image::imageops::overlay(&mut self.main, &mut im, x, y);
        draw_filled_rect_mut(
            self.get_targ(),
            imageproc::rect::Rect::at(xx as i32, yy as i32).of_size(ww as u32, hh as u32),
            image::Rgba([255, 255, 255, 255]),
        );
    }
    pub fn pixel(&mut self, x: u32, y: u32, r: f32, g: f32, b: f32, a: f32) {
        self.get_targ().get_pixel_mut(x, y).0 = [
            (r * 255.) as u8,
            (g * 255.) as u8,
            (b * 255.) as u8,
            (a * 255.) as u8,
        ];
    }

    pub fn line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32) {
        let width = self.size[0];
        let height = self.size[1];

        let xx1 = x1.max(0.) * width as f32;
        let yy1 = y1.max(0.) * height as f32;
        let xx2 = x2.max(0.) * width as f32;
        let yy2 = y2.max(0.) * height as f32;
        // let mut im = RgbaImage::new(w, h);
        // image::imageops::overlay(&mut self.main, &mut im, x, y);

        let white = image::Rgba([255, 255, 255, 255]);
        imageproc::drawing::draw_line_segment_mut(self.get_targ(), (xx1, yy1), (xx2, yy2), white);
        // draw_line_mut(
        //     &mut self.main,
        //     imageproc::point::Point::new(xx1 as i32, yy1 as i32),
        //     imageproc::point::Point::new(xx2 as i32, yy2 as i32),
        //     image::Rgba([255, 255, 255, 255]),
        // );
    }

    fn get_targ(&mut self) -> &mut RgbaImage {
        if self.target_sky {
            self.dirty_sky = true;
            &mut self.sky
        } else {
            self.dirty = true;
            &mut self.main
        }
    }

    /* Clean off the main raster */
    pub fn clean(&mut self) {
        self.main = RgbaImage::new(self.size[0], self.size[1]);
        self.dirty = true;
    }

    pub fn enable_console(&mut self) {
        self.text = crate::log::get(
            (self.size[0] / (LETTER_SIZE + 1) - 2) as usize,
            (self.size[1] / (LETTER_SIZE + 1) - 8) as usize,
        );
        self.output = true;
        self.apply_console_out_text();
    }
    pub fn disable_console(&mut self) {
        self.text = "".to_string();
        self.output = false;
        self.apply_console_out_text();
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
                (self.size[0] / (LETTER_SIZE + 1) - 2) as usize,
                (self.size[1] / (LETTER_SIZE + 1) - 8) as usize,
            );
            self.apply_console_out_text();
            crate::log::clean();
        }
        if self.dirty {
            let raster = if self.output {
                &self.console
            } else {
                &self.main
            };
            crate::texture::write_tex(queue, &self.overlay_texture, raster);
            self.dirty = false;
        }
        if self.dirty_sky {
            crate::texture::write_tex(queue, &self.sky_texture, &self.sky);
            self.dirty_sky = false;
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
    let h = (GUI_DIM as f32 / aspect).ceil() as u32;
    println!("aspect {}", h);
    let mut img: RgbaImage = ImageBuffer::new(GUI_DIM, h);
    img.put_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
    let out = crate::texture::make_tex(device, queue, &img);
    (out.0, out.1, out.2, img)
}

fn log(str: String) {
    crate::log::log(format!("📺gui::{}", str));
}

use std::{borrow::Borrow, rc::Rc};

use glam::Vec4;
use rand::Rng;
// use tracy::frame;

use crate::{
    command::NumCouple,
    log::{LogType, Loggy},
    texture::TexManager,
};
use image::{ImageBuffer, RgbaImage};
use imageproc::drawing::{draw_filled_rect, draw_filled_rect_mut};
use wgpu::{Device, Queue, Sampler, Texture, TextureView};

const LETTER_SIZE: u32 = 8;
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
    pub console_background: image::Rgba<u8>,
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
        loggy: &mut Loggy,
    ) -> Gui {
        let d = gui_img.dimensions();

        let letters = match crate::texture::load_img(&"6x6-8unicode.png".to_string(), loggy) {
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

                let d = include_bytes!(concat!("..", sp!(), "assets", sp!(), "6x6-8unicode.png"));
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
            console_background: image::Rgba([255, 255, 255, 0]),
        }
    }

    //pub fn type()
    pub fn add_text(&mut self, str: String) {
        self.text = format!("{}\n{}", self.text, str);
        self.apply_console_out_text();
    }

    // DEV is this still useful? it numbers tiles on a 16x16 grid
    pub fn add_img(&self, str: &String, loggy: &mut Loggy) {
        match crate::texture::load_img(str, loggy) {
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
            }
            Err(e) => {
                loggy.log(LogType::TextureError, &e.to_string());
            }
        }
    }

    // pub fn direct_text(&mut self, txt: &String, onto_console: bool, x: NumCouple, y: NumCouple) {
    //     let targ = if onto_console {
    //         &mut self.console
    //     } else {
    //         if self.target_sky {
    //             self.dirty_sky = true;
    //             &mut self.sky
    //         } else {
    //             self.dirty = true;
    //             &mut self.main
    //         }
    //     };
    //     let xx = (x.1 * if x.0 { 1. } else { self.size[0] as f32 }) as i64;
    //     let yy = (y.1 * if y.0 { 1. } else { self.size[1] as f32 }) as i64;

    //     for (i, line) in txt.lines().enumerate() {
    //         let ly = yy + i as i64 * (LETTER_SIZE as i64 + 2);
    //         let mut lx = xx + (LETTER_SIZE) as i64;
    //         for c in line.chars() {
    //             let mut ind = c as u32;
    //             if ind > 255 {
    //                 ind = 255;
    //             }
    //             let index_x = ind % 16;
    //             let index_y = ind / 16;
    //             let sub = image::imageops::crop_imm(
    //                 &self.letters,
    //                 index_x * LETTER_SIZE,
    //                 index_y * LETTER_SIZE,
    //                 LETTER_SIZE,
    //                 LETTER_SIZE,
    //             );
    //             image::imageops::overlay(targ, &mut sub.to_image(), lx, ly);
    //             lx += (LETTER_SIZE + 1) as i64;
    //         }
    //     }
    // }

    pub fn draw_image(
        &mut self,
        tex_manager: &mut TexManager,
        image: &String,
        onto_console: bool,
        x: NumCouple,
        y: NumCouple,
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

        let w = tex_manager.atlas.width();
        let h = tex_manager.atlas.height();
        let sub = image::imageops::crop_imm(
            &mut tex_manager.atlas,
            (source.x * w as f32) as u32,
            (source.y * h as f32) as u32,
            (source.z * w as f32) as u32,
            (source.w * h as f32) as u32,
        );

        direct_image(targ, &sub.to_image(), x, y, self.size[0], self.size[1]);

        // *targ = image::imageops::huerotate(targ, rand::thread_rng().gen_range(0..360));
        // self.dirty = true;
    }

    pub fn set_console_background_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
        self.console_background = image::Rgba([r, g, b, a])
    }
    pub fn apply_console_out_text(&mut self) {
        let im = RgbaImage::new(self.size[0], self.size[1]);
        image::imageops::replace(&mut self.console, &im, 0, 0);

        for (i, line) in self.text.lines().enumerate() {
            let y = i as i64 * (LETTER_SIZE as i64 + 2);
            let mut x = (LETTER_SIZE) as i64;
            for c in line.chars() {
                let mut ind = c as u32;

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
                    self.console_background,
                );
                //sub.to_image().
                image::imageops::overlay(&mut self.console, &mut sub.to_image(), x, y);
                x += (LETTER_SIZE + 1) as i64;
            }
        }

        // self.console =
        //     image::imageops::huerotate(&mut self.console, rand::thread_rng().gen_range(0..360));
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

    pub fn pixel(&mut self, x: u32, y: u32, r: f32, g: f32, b: f32, a: f32) {
        self.get_targ().get_pixel_mut(x, y).0 = [
            (r * 255.) as u8,
            (g * 255.) as u8,
            (b * 255.) as u8,
            (a * 255.) as u8,
        ];
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

    pub fn enable_console(&mut self, loggy: &Loggy) {
        self.text = loggy.get(
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
    // pub fn overlay_image(&mut self, image: &RgbaImage) {
    //     image::imageops::overlay(&mut self.main, image, 0, 0);
    //     self.dirty = true;
    // }
    pub fn replace_image(&mut self, img: RgbaImage, is_sky: bool) {
        if is_sky {
            self.sky = img;
            self.dirty_sky = true;
        } else {
            self.main = img;
            self.dirty = true;
        }
    }
    // pub fn toggle_output(&mut self) {
    //     if self.output {
    //         self.disable_output();
    //     } else {
    //         self.enable_output();
    //     }
    // }
    // fn draw_img();
    pub fn render(&mut self, queue: &Queue, time: f32, loggy: &mut Loggy) {
        //let mut rng = rand::thread_rng();
        // frame!("gui start");
        self.time = time;
        if self.output && loggy.is_dirty_and_listen() {
            self.text = loggy.get(
                (self.size[0] / (LETTER_SIZE + 1) - 2) as usize,
                (self.size[1] / (LETTER_SIZE + 1) - 8) as usize,
            );
            self.apply_console_out_text();
            loggy.clean();
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
    }

    pub fn make_morsel(&self) -> PreGuiMorsel {
        (
            self.letters.clone(),
            self.main.clone(),
            self.sky.clone(),
            self.size,
        )
    }
}

pub type PreGuiMorsel = (RgbaImage, RgbaImage, RgbaImage, [u32; 2]);
pub struct GuiMorsel {
    pub letters: Rc<RgbaImage>,
    dirty: bool,
    dirty_sky: bool,
    pub main: RgbaImage,
    pub sky: RgbaImage,
    target_sky: bool,
    pub size: [u32; 2],
}

impl GuiMorsel {
    pub fn new((letters, main, sky, size): PreGuiMorsel) -> Self {
        // letters: Rc<RgbaImage>, main: RgbaImage, sky: RgbaImage, size: [u32; 2]
        Self {
            letters: Rc::new(letters),
            dirty: true,
            dirty_sky: true,
            main,
            sky,
            target_sky: false,
            size,
        }
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

    pub fn rect(&mut self, x: NumCouple, y: NumCouple, w: NumCouple, h: NumCouple, c: Vec4) {
        let width = self.size[0];
        let height = self.size[1];

        direct_rect(self.get_targ(), width, height, x, y, w, h, c)
    }

    pub fn line(&mut self, x1: NumCouple, y1: NumCouple, x2: NumCouple, y2: NumCouple, c: Vec4) {
        let width = self.size[0];
        let height = self.size[1];

        direct_line(self.get_targ(), width, height, x1, y1, x2, y2, c)
    }

    pub fn text(&mut self, txt: &str, x: NumCouple, y: NumCouple, c: Vec4) {
        let targ = if self.target_sky {
            self.dirty_sky = true;
            &mut self.sky
        } else {
            self.dirty = true;
            &mut self.main
        };

        direct_text(
            targ,
            &self.letters,
            self.size[0],
            self.size[1],
            txt,
            x,
            y,
            c,
        )
    }

    pub fn pixel(&mut self, x: u32, y: u32, r: f32, g: f32, b: f32, a: f32) {
        self.get_targ().get_pixel_mut(x, y).0 = [
            (r * 255.) as u8,
            (g * 255.) as u8,
            (b * 255.) as u8,
            (a * 255.) as u8,
        ];
    }

    pub fn draw_image(&mut self, image: &RgbaImage, x: NumCouple, y: NumCouple) {
        let xx = if x.0 { x.1 } else { x.1 * self.size[0] as f32 };
        let yy = if y.0 { y.1 } else { y.1 * self.size[1] as f32 };
        image::imageops::overlay(self.get_targ(), image, xx as i64, yy as i64);

        // *targ = image::imageops::huerotate(targ, rand::thread_rng().gen_range(0..360));
        // self.dirty = true;
    }

    /* Clean off the main raster */
    pub fn clean(&mut self) {
        // self.main.

        self.main = RgbaImage::new(self.size[0], self.size[1]);
        self.dirty = true;
    }

    pub fn target_gui(&mut self) {
        self.target_sky = false;
    }
    pub fn target_sky(&mut self) {
        self.target_sky = true;
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

    pub fn send_state(&mut self) -> Option<(RgbaImage, bool)> {
        if self.dirty {
            self.dirty = false;
            return Some((self.main.clone(), false));
        } else if self.dirty_sky {
            self.dirty_sky = false;
            return Some((self.sky.clone(), true));
        }
        None
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

pub fn direct_rect(
    target: &mut RgbaImage,
    width: u32,
    height: u32,
    x: NumCouple,
    y: NumCouple,
    w: NumCouple,
    h: NumCouple,
    c: Vec4,
) {
    let xx = x.1.max(0.) * if x.0 { 1. } else { width as f32 };
    let yy = y.1.max(0.) * if y.0 { 1. } else { height as f32 };
    let ww = (w.1.max(0.) * if w.0 { 1. } else { width as f32 }).max(1.);
    let hh = (h.1.max(0.) * if h.0 { 1. } else { height as f32 }).max(1.);
    draw_filled_rect_mut(
        target,
        imageproc::rect::Rect::at(xx as i32, yy as i32).of_size(ww as u32, hh as u32),
        image::Rgba([
            (c.x * 255.).floor() as u8,
            (c.y * 255.).floor() as u8,
            (c.z * 255.).floor() as u8,
            (c.w * 255.).floor() as u8,
        ]),
    );
}

pub fn direct_line(
    target: &mut RgbaImage,
    width: u32,
    height: u32,
    x1: NumCouple,
    y1: NumCouple,
    x2: NumCouple,
    y2: NumCouple,
    c: Vec4,
) {
    let xx1 = x1.1.max(0.) * if x1.0 { 1. } else { width as f32 };
    let yy1 = y1.1.max(0.) * if y1.0 { 1. } else { height as f32 };
    let xx2 = x2.1.max(0.) * if x2.0 { 1. } else { width as f32 };
    let yy2 = y2.1.max(0.) * if y2.0 { 1. } else { height as f32 };

    let color = image::Rgba([
        (c.x * 255.).floor() as u8,
        (c.y * 255.).floor() as u8,
        (c.z * 255.).floor() as u8,
        (c.w * 255.).floor() as u8,
    ]);
    imageproc::drawing::draw_line_segment_mut(target, (xx1, yy1), (xx2, yy2), color);
}

pub fn direct_text(
    target: &mut RgbaImage,
    letters: &Rc<RgbaImage>,
    width: u32,
    height: u32,
    txt: &str,
    x: NumCouple,
    y: NumCouple,
    c: Vec4,
) {
    let xx = (x.1 * if x.0 { 1. } else { width as f32 }) as i64;
    let yy = (y.1 * if y.0 { 1. } else { height as f32 }) as i64;

    for (i, line) in txt.lines().enumerate() {
        let ly = yy + i as i64 * (LETTER_SIZE as i64 + 2);
        let mut lx = xx + (LETTER_SIZE) as i64;
        for c in line.chars() {
            let mut ind = c as u32;
            if ind > 255 {
                ind = 255;
            }
            let index_x = ind % 16;
            let index_y = ind / 16;
            let sub: image::SubImage<&RgbaImage> = image::imageops::crop_imm(
                letters.borrow(),
                index_x * LETTER_SIZE,
                index_y * LETTER_SIZE,
                LETTER_SIZE,
                LETTER_SIZE,
            );
            image::imageops::overlay(target, &mut sub.to_image(), lx, ly);
            lx += (LETTER_SIZE + 1) as i64;
        }
    }
}

pub fn direct_image(
    target: &mut RgbaImage,
    source: &RgbaImage,
    x: NumCouple,
    y: NumCouple,
    width: u32,
    height: u32,
) {
    let xx = if x.0 { x.1 } else { x.1 * width as f32 };
    let yy = if y.0 { y.1 } else { y.1 * height as f32 };

    image::imageops::overlay(target, source, xx as i64, yy as i64);
}

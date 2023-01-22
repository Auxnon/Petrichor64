use std::{borrow::Borrow, rc::Rc};

use glam::Vec4;

use crate::{
    global::GuiParams,
    log::{LogType, Loggy},
    lua_define::LuaResponse,
    texture::{TexManager, TexTuple},
};
use image::{ImageBuffer, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut};
use wgpu::{BindGroup, Device, Queue, Sampler, Texture, TextureView};

const LETTER_SIZE: u32 = 8;

// pub enum GuiUnit {
//     Percent(f32),
//     Pixel(u32),
//     AspectPixel(f32),
//     AspectPercent(f32),
//     ReversePixel(u32),
//     ReversePercent(f32),
// }

pub struct Gui {
    pub gui_pipeline: wgpu::RenderPipeline,
    pub sky_pipeline: wgpu::RenderPipeline,
    pub gui_group: wgpu::BindGroup,
    pub sky_group: wgpu::BindGroup,
    // pub model: Arc<OnceCell<Model>>,
    pub overlay_texture: TexTuple,
    pub sky_texture: TexTuple,
    text: String,
    /** main game rendered gui raster to stay in memory if toggled to console */
    main: RgbaImage,
    /** console raster with current output to stay in memory*/
    console: RgbaImage,
    /** skybox raster */
    sky: RgbaImage,

    time: f32,
    pub letters: RgbaImage,
    size: [u32; 2],
    pub console_background: image::Rgba<u8>,
    // console_dity: bool,
    // main_dirty: bool,
    dirty: bool,
    dirty_sky: bool,
    target_sky: bool,
    output: bool,
    pub local_gui_params: GuiParams,
}

impl Gui {
    pub fn new(
        gui_pipeline: wgpu::RenderPipeline,
        sky_pipeline: wgpu::RenderPipeline,
        main_bind_group_layout: &wgpu::BindGroupLayout,
        uniform_buf: &wgpu::Buffer,
        // gui_group: wgpu::BindGroup,
        // overlay_texture: TexTuple,
        // sky_group: wgpu::BindGroup,
        // sky_texture: TexTuple,
        device: &Device,
        queue: &Queue,
        gui_scaled: (u32, u32),
        // gui_img: RgbaImage,
        loggy: &mut Loggy,
    ) -> Gui {
        let (gui_bundle, gui_image) = init_image(&device, &queue, gui_scaled);

        let (sky_bundle, sky_image) = init_image(&device, &queue, gui_scaled);

        let (gui_group, sky_group) = rebuild_group(
            &gui_bundle,
            &sky_bundle,
            device,
            main_bind_group_layout,
            uniform_buf,
        );

        let d = gui_image.dimensions();

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
            overlay_texture: gui_bundle,
            sky_texture: sky_bundle,
            console: gui_image.clone(),
            sky: gui_image.clone(),
            dirty_sky: false,
            target_sky: false,
            main: gui_image,
            text: "".to_string(),
            letters,
            time: 0.,
            size: [d.0, d.1],
            dirty: true,
            output: false,
            console_background: image::Rgba([255, 255, 255, 0]),
            local_gui_params: GuiParams::new(),
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

    // pub fn direct_text(&mut self, txt: &String, onto_console: bool, x: LuaResponse, y: LuaResponse) {
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
        x: LuaResponse,
        y: LuaResponse,
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
        println!("cleaning {:?}", self.size);
        self.main = RgbaImage::new(self.size[0], self.size[1]);
        self.dirty = true;
    }

    pub fn get_console_size(&self) -> (u32, u32) {
        (
            (self.size[0] / (LETTER_SIZE + 1) - 2),
            (self.size[1] / (LETTER_SIZE + 1) - 8),
        )
    }

    pub fn enable_console(&mut self, loggy: &Loggy) {
        self.text = loggy.get();
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
        println!("replacing image of size {:?}", img.dimensions());
        if is_sky {
            self.sky = img;
            self.dirty_sky = true;
        } else {
            self.main = img;
            self.dirty = true;
        }
    }
    pub fn resize(
        &mut self,
        size: (u32, u32),
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        main_bind_group_layout: &wgpu::BindGroupLayout,
        uniform_buf: &wgpu::Buffer,
    ) {
        self.size = [size.0, size.1];
        // TODO
        self.main = image::imageops::resize(
            &self.main,
            size.0,
            size.1,
            image::imageops::FilterType::Nearest,
        );
        self.console = image::imageops::resize(
            &self.console,
            size.0,
            size.1,
            image::imageops::FilterType::Nearest,
        );
        self.sky = image::imageops::resize(
            &self.sky,
            size.0,
            size.1,
            image::imageops::FilterType::Nearest,
        );

        self.overlay_texture = crate::texture::make_tex(device, queue, &self.main);
        self.sky_texture = crate::texture::make_tex(device, queue, &self.sky);

        let (gui_group, sky_group) = rebuild_group(
            &self.overlay_texture,
            &self.sky_texture,
            device,
            main_bind_group_layout,
            uniform_buf,
        );
        self.gui_group = gui_group;
        self.sky_group = sky_group;

        // self.main.self.main = RgbaImage::new(size[0], size[1]);
        // self.console = RgbaImage::new(size[0], size[1]);
        // self.sky = RgbaImage::new(size[0], size[1]);
        println!(
            "resize to {:?} {:?} {:?} {:?}",
            size,
            self.main.dimensions(),
            self.console.dimensions(),
            self.sky.dimensions()
        );

        self.apply_console_out_text();
        self.dirty = true;
        self.dirty_sky = true;
    }

    pub fn change_layout(&mut self, gui_params: GuiParams) {
        self.local_gui_params = gui_params;
    }

    pub fn render(&mut self, queue: &Queue, time: f32, loggy: &mut Loggy) {
        self.time = time;
        if loggy.is_dirty_and_listen() && self.output {
            self.text = loggy.get();
            self.apply_console_out_text();
            loggy.clean();
        }
        if self.dirty {
            let raster = if self.output {
                &self.console
            } else {
                &self.main
            };
            println!(
                "raster size console {} {:?} stored size{:?}",
                self.output,
                raster.dimensions(),
                self.size
            );
            crate::texture::write_tex(queue, &self.overlay_texture.texture, raster);
            self.dirty = false;
        }
        if self.dirty_sky {
            crate::texture::write_tex(queue, &self.sky_texture.texture, &self.sky);
            self.dirty_sky = false;
        }
    }

    pub fn make_morsel(&self) -> PreGuiMorsel {
        //DEV debug
        println!("morsel size {:?} ", self.size);
        (
            self.letters.clone(),
            RgbaImage::new(self.size[0], self.size[1]),
            RgbaImage::new(self.size[0], self.size[1]),
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

    pub fn fill(&mut self, c: Vec4) {
        let width = self.size[0];
        let height = self.size[1];
        direct_fill(self.get_targ(), width, height, c);
    }

    pub fn rect(
        &mut self,
        x: LuaResponse,
        y: LuaResponse,
        w: LuaResponse,
        h: LuaResponse,
        c: Vec4,
        corner: Option<LuaResponse>,
    ) {
        let width = self.size[0];
        let height = self.size[1];

        direct_rect(self.get_targ(), width, height, x, y, w, h, c, corner)
    }

    pub fn line(
        &mut self,
        x1: LuaResponse,
        y1: LuaResponse,
        x2: LuaResponse,
        y2: LuaResponse,
        c: Vec4,
    ) {
        let width = self.size[0];
        let height = self.size[1];

        direct_line(self.get_targ(), width, height, x1, y1, x2, y2, c)
    }

    pub fn text(&mut self, txt: &str, x: LuaResponse, y: LuaResponse, c: Vec4) {
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

    pub fn pixel(&mut self, x: u32, y: u32, rgb: Vec4) {
        let s = self.size;
        println!("morself gui {} {}", s[0], s[1]);

        self.get_targ()
            .get_pixel_mut(x.min(s[0] - 1), y.min(s[1] - 1))
            .0 = [
            (rgb.x * 255.) as u8,
            (rgb.y * 255.) as u8,
            (rgb.z * 255.) as u8,
            (rgb.w * 255.) as u8,
        ];
    }
    pub fn resize(&mut self, w: u32, h: u32) {
        self.size = [w, h];
        println!("morself gui {} {}", w, h);
        self.main = image::imageops::resize(&self.main, w, h, image::imageops::FilterType::Nearest);
        self.sky = image::imageops::resize(&self.sky, w, h, image::imageops::FilterType::Nearest);
        self.dirty = true;
        self.dirty_sky = true;
    }

    pub fn draw_image(&mut self, image: &RgbaImage, x: LuaResponse, y: LuaResponse) {
        let xx = eval(x, self.size[0]);
        let yy = eval(y, self.size[1]);

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

    pub fn new_image(w: u32, h: u32) -> RgbaImage {
        RgbaImage::new(w, h)
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

pub fn eval(val: LuaResponse, l: u32) -> i32 {
    match val {
        LuaResponse::Integer(i) => {
            if i < 0 {
                l as i32 - i
            } else {
                i
            }
        }
        LuaResponse::Number(f) => {
            let ff = (f * l as f64) as i32;
            if f < 0. {
                l as i32 - ff
            } else {
                ff
            }
        }
        LuaResponse::String(s) => {
            let st = s.trim();
            if st.starts_with("=") {
                let mut add_op = true;
                let mut res = 0;
                st[1..st.len()].split_inclusive(['-', '+']).for_each(|p| {
                    let op = add_op;
                    let seg = if p.ends_with("-") {
                        add_op = false;
                        p[0..p.len() - 1].trim()
                    } else if p.ends_with("+") {
                        add_op = true;
                        p[0..p.len() - 1].trim()
                    } else {
                        p.trim()
                    };

                    let n = adeval(seg, l);
                    if op {
                        println!("{} {} + {}", p, res, n);
                        res += n;
                    } else {
                        println!("{} {} - {}", p, res, n);
                        res -= n;
                    }
                });
                return res;
            } else {
                adeval(st, l)
            }
        }
        _ => 0,
    }
}

pub fn rebuild_group(
    gui_bundle: &TexTuple,
    sky_bundle: &TexTuple,
    device: &Device,
    main_bind_group_layout: &wgpu::BindGroupLayout,
    uniform_buf: &wgpu::Buffer,
) -> (BindGroup, BindGroup) {
    let gui_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &main_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&gui_bundle.view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&gui_bundle.sampler),
            },
        ],
        label: None,
    });

    let sky_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &main_bind_group_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&sky_bundle.view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&sky_bundle.sampler),
            },
        ],
        label: None,
    });
    (gui_group, sky_group)
}

fn adeval(st: &str, l: u32) -> i32 {
    if st.ends_with("%") {
        if st.ends_with("@%") {
            // let n = st[0..st.len() - 2].parse::<f32>().unwrap_or(0.);
            // (l as f32 * (n / 100.)) as i32
            0
        } else {
            let n = st[0..st.len() - 1].parse::<f32>().unwrap_or(0.);
            (l as f32 * (n / 100.)) as i32
        }
    } else if st.ends_with("@") {
        let n = st[0..st.len() - 1].parse::<u32>().unwrap_or(0);
        0
    } else {
        st.parse::<i32>().unwrap_or(0)
    }
}

pub fn init_image(device: &Device, queue: &Queue, size: (u32, u32)) -> (TexTuple, RgbaImage) {
    // println!("aspect {}", h);
    let mut img: RgbaImage = ImageBuffer::new(size.0, size.1);
    // img.put_pixel(1, 1, image::Rgba([0, 255, 0, 255]));
    let out = crate::texture::make_tex(device, queue, &img);
    (out, img)
}

pub fn direct_rect(
    target: &mut RgbaImage,
    width: u32,
    height: u32,
    x: LuaResponse,
    y: LuaResponse,
    w: LuaResponse,
    h: LuaResponse,
    c: Vec4,
    corner: Option<LuaResponse>,
) {
    let xx = eval(x, width);
    let yy = eval(y, height);
    let ww = eval(w, width).max(1) as u32;
    let hh = eval(h, height).max(1) as u32;
    let color = image::Rgba([
        (c.x * 255.).floor() as u8,
        (c.y * 255.).floor() as u8,
        (c.z * 255.).floor() as u8,
        (c.w * 255.).floor() as u8,
    ]);
    match corner {
        Some(c) => {
            let maxa = (ww.min(hh) / 2) as i32;
            let mut radius = eval(c, maxa as u32) as i32;

            if radius > maxa {
                radius = maxa;
            }
            let ww2 = ww as i32;
            let hh2 = hh as i32;

            let cx1 = xx + (radius);
            let cx2 = xx + (ww2 - radius - 1); // -1 is error correction, why do we need it? -\_(ツ)_/-
            let cy1 = yy + radius;
            let cy2 = yy + (hh2 - radius - 1);
            draw_filled_circle_mut(target, (cx1, cy1), radius, color);
            draw_filled_circle_mut(target, (cx1, cy2), radius, color);
            draw_filled_circle_mut(target, (cx2, cy1), radius, color);
            draw_filled_circle_mut(target, (cx2, cy2), radius, color);

            draw_filled_rect_mut(
                target,
                imageproc::rect::Rect::at(xx, cy1).of_size(ww, (cy2 - cy1) as u32),
                color,
            );
            draw_filled_rect_mut(
                target,
                imageproc::rect::Rect::at(cx1, yy).of_size((cx2 - cx1) as u32, hh),
                color,
            );
        }
        None => {
            draw_filled_rect_mut(
                target,
                imageproc::rect::Rect::at(xx, yy).of_size(ww, hh),
                color,
            );
        }
    }
}

pub fn direct_line(
    target: &mut RgbaImage,
    width: u32,
    height: u32,
    x1: LuaResponse,
    y1: LuaResponse,
    x2: LuaResponse,
    y2: LuaResponse,
    c: Vec4,
) {
    let xx1 = eval(x1, width);
    let yy1 = eval(y1, height);
    let xx2 = eval(x2, width);
    let yy2 = eval(y2, height);

    let color = image::Rgba([
        (c.x * 255.).floor() as u8,
        (c.y * 255.).floor() as u8,
        (c.z * 255.).floor() as u8,
        (c.w * 255.).floor() as u8,
    ]);
    imageproc::drawing::draw_line_segment_mut(
        target,
        (xx1 as f32, yy1 as f32),
        (xx2 as f32, yy2 as f32),
        color,
    );
}

pub fn direct_text(
    target: &mut RgbaImage,
    letters: &Rc<RgbaImage>,
    width: u32,
    height: u32,
    txt: &str,
    x: LuaResponse,
    y: LuaResponse,
    c: Vec4,
) {
    let xx = eval(x, width);
    let yy = eval(y, height);

    for (i, line) in txt.lines().enumerate() {
        let ly = yy + i as i32 * (LETTER_SIZE as i32 + 2);
        let mut lx = xx + (LETTER_SIZE) as i32;
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
            image::imageops::overlay(target, &mut sub.to_image(), lx.into(), ly.into());
            lx += (LETTER_SIZE + 1) as i32;
        }
    }
}

pub fn direct_image(
    target: &mut RgbaImage,
    source: &RgbaImage,
    x: LuaResponse,
    y: LuaResponse,
    width: u32,
    height: u32,
) {
    let xx = eval(x, width);
    let yy = eval(y, height);

    image::imageops::overlay(target, source, xx as i64, yy as i64);
}

pub fn direct_fill(target: &mut RgbaImage, width: u32, height: u32, c: Vec4) {
    let color = image::Rgba([
        (c.x * 255.).floor() as u8,
        (c.y * 255.).floor() as u8,
        (c.z * 255.).floor() as u8,
        (c.w * 255.).floor() as u8,
    ]);
    draw_filled_rect_mut(
        target,
        imageproc::rect::Rect::at(0, 0).of_size(width, height),
        color,
    );
}

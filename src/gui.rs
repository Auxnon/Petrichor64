use std::{borrow::Borrow, rc::Rc};

use glam::{vec4, Vec4};

use crate::{global::GuiParams, log::Loggy, lua_define::LuaResponse};

#[cfg(feature = "headed")]
use crate::texture::TexTuple;

use image::{imageops::ColorMap, ImageBuffer, Rgba, RgbaImage};
use imageproc::drawing::{draw_filled_circle_mut, draw_filled_rect_mut};
#[cfg(feature = "headed")]
use wgpu::{BindGroup, Device, Queue};

const LETTER_SIZE: u32 = 8;
// const NOTIF_WIDTH: u32 = 320;

pub enum ScreenIndex {
    System = 0,
    Primary = 1,
    Secondary = 2,
    Trinary = 3,
    Sky = 4,
}

#[cfg(feature = "headed")]
struct ScreenLayer {
    pub texture: TexTuple,
    pub image: RgbaImage,
    pub dirty: bool,
}
#[cfg(feature = "headed")]
impl ScreenLayer {
    pub fn resize(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, size: &[u32; 2]) {
        self.image = RgbaImage::new(size[0], size[1]);
        self.dirty = true;
        self.image = image::imageops::resize(
            &mut self.image,
            size[0],
            size[1],
            image::imageops::FilterType::Nearest,
        );
        self.texture = crate::texture::make_tex(device, queue, &self.image);
    }
    pub fn check_render(&mut self, queue: &wgpu::Queue) {
        if self.dirty {
            crate::texture::write_tex(queue, &self.texture.texture, &self.image);
            self.dirty = false;
        }
    }
}

pub struct Gui {
    #[cfg(feature = "headed")]
    pub gui_pipeline: wgpu::RenderPipeline,
    #[cfg(feature = "headed")]
    pub sky_pipeline: wgpu::RenderPipeline,
    #[cfg(feature = "headed")]
    pub gui_group: wgpu::BindGroup,
    #[cfg(feature = "headed")]
    pub gui_aux_group: wgpu::BindGroup,
    #[cfg(feature = "headed")]
    pub sky_group: wgpu::BindGroup,
    #[cfg(feature = "headed")]
    system_layer: ScreenLayer,
    #[cfg(feature = "headed")]
    primary_layer: ScreenLayer,
    #[cfg(feature = "headed")]
    secondary_layer: ScreenLayer,
    #[cfg(feature = "headed")]
    trinary_layer: ScreenLayer,
    #[cfg(feature = "headed")]
    sky_layer: ScreenLayer,
    // primary_texture: TexTuple,
    // secondary_texture: TexTuple,
    // trinary_texture: TexTuple,
    // system_texture: TexTuple,
    // pub sky_texture: TexTuple,
    console_string: String,
    /** main game rendered gui raster to stay in memory if toggled to console */
    // primary_image: RgbaImage,
    // secondary_image: RgbaImage,
    // trinary_image: RgbaImage,
    /** console or notifications raster with current output to stay in memory*/
    // system_image: RgbaImage,
    /** skybox raster */
    // sky_image: RgbaImage,
    time: f32,
    pub letters: RgbaImage,
    size: [u32; 2],
    console_background_color: image::Rgba<u8>,
    // console_dity: bool,
    // main_dirty: bool,
    // dirty: bool,
    // dirty_sky: bool,
    // target_sky: bool,
    /** output console to gui */
    output_console: bool,
    active_notifs: Vec<Notif>,
    notifcations_enabled: bool,
    notifications_dirty: bool,
    pub local_gui_params: GuiParams,
}

impl Gui {
    #[cfg(feature = "headed")]
    pub fn new(
        gfx: &crate::gfx::Gfx,
        gui_pipeline: wgpu::RenderPipeline,
        sky_pipeline: wgpu::RenderPipeline,
        gui_scaled: (u32, u32),
        loggy: &mut Loggy,
    ) -> Gui {
        let (system_texture, system_image) = init_image(&gfx.device, &gfx.queue, gui_scaled);
        let (primary_texture, primary_image) = init_image(&gfx.device, &gfx.queue, gui_scaled);
        let (secondary_texture, secondary_image) = init_image(&gfx.device, &gfx.queue, gui_scaled);
        let (trinary_texture, trinary_image) = init_image(&gfx.device, &gfx.queue, gui_scaled);

        let (sky_bundle, sky_image) = init_image(&gfx.device, &gfx.queue, gui_scaled);

        let (gui_group, gui_aux_group, sky_group) = rebuild_group(
            [
                &system_texture,
                &primary_texture,
                &secondary_texture,
                &trinary_texture,
                &sky_bundle,
            ],
            &gfx.device,
            &gfx.main_layout,
            &gfx.gui_aux_layout,
            &gfx.uniform_buf,
        );

        let d = system_image.dimensions();

        let letters = Self::letter_init(loggy);

        // let raster: RgbaImage = ImageBuffer::new(1024, 1024);

        Gui {
            gui_pipeline,
            sky_pipeline,
            gui_group,
            gui_aux_group,
            sky_group,
            system_layer: ScreenLayer {
                // index: ScreenIndex::System,
                texture: system_texture,
                image: system_image,
                dirty: true,
            },
            primary_layer: ScreenLayer {
                // index: ScreenIndex::Primary,
                texture: primary_texture,
                image: primary_image,
                dirty: true,
            },
            secondary_layer: ScreenLayer {
                // index: ScreenIndex::Secondary,
                texture: secondary_texture,
                image: secondary_image,
                dirty: true,
            },
            trinary_layer: ScreenLayer {
                // index: ScreenIndex::Trinary,
                texture: trinary_texture,
                image: trinary_image,
                dirty: true,
            },
            sky_layer: ScreenLayer {
                // index: ScreenIndex::Sky,
                texture: sky_bundle,
                image: sky_image,
                dirty: true,
            },

            console_string: "".to_string(),
            letters,
            time: 0.,
            size: [d.0, d.1],
            output_console: false,
            console_background_color: image::Rgba([47, 47, 47, 255]),
            local_gui_params: GuiParams::new(),
            active_notifs: vec![],
            notifcations_enabled: true,
            notifications_dirty: true,
        }
    }

    #[cfg(not(feature = "headed"))]
    pub fn new(gui_scaled: (u32, u32), loggy: &mut Loggy) -> Gui {
        Gui {
            console_string: "".to_string(),
            letters: Gui::letter_init(loggy),
            time: 0.,
            size: [gui_scaled.0, gui_scaled.1],
            output_console: false,
            console_background_color: image::Rgba([47, 47, 47, 255]),
            local_gui_params: GuiParams::new(),
            active_notifs: vec![],
            notifcations_enabled: false,
            notifications_dirty: true,
        }
    }

    fn letter_init(loggy: &mut Loggy) -> RgbaImage {
        match crate::asset::load_img(&"6x6-8unicode.png".to_string(), loggy) {
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
                crate::asset::load_img_from_buffer(d).unwrap().into_rgba8()
            }
        }
    }

    pub fn add_text(&mut self, str: String) {
        self.console_string = format!("{}\n{}", self.console_string, str);
        #[cfg(feature = "headed")]
        self.apply_console_out_text();
    }

    // DEV is this still useful? it numbers tiles on a 16x16 grid
    // pub fn add_img(&self, str: &String, loggy: &mut Loggy) {
    //     match crate::asset::load_img(str, loggy) {
    //         Ok(t) => {
    //             let mut im = t.to_rgba8();
    //             let w = im.width() / 16;
    //             let h = im.height() / 16;
    //             for x in 0..w {
    //                 for y in 0..h {
    //                     let n = x + y * w;
    //                     let digits: Vec<_> = n
    //                         .to_string()
    //                         .chars()
    //                         .map(|d| d.to_digit(10).unwrap())
    //                         .collect();
    //                     for (i, d) in digits.into_iter().enumerate() {
    //                         let sub = image::imageops::crop_imm(
    //                             &self.letters,
    //                             d * LETTER_SIZE,
    //                             12,
    //                             LETTER_SIZE,
    //                             LETTER_SIZE,
    //                         );
    //                         // let imm: &ImageBuffer<Rgba<u8>, Vec<u8>> = sub.inner();
    //                         image::imageops::replace(
    //                             &mut im,
    //                             &mut sub.to_image(),
    //                             (x * 16 + i as u32 * (LETTER_SIZE + 1)).into(),
    //                             (y * 16).into(),
    //                         );
    //                     }

    //                     //image::imageops::overlay(&mut self.img, &sub, x, y);
    //                 }
    //             }
    //         }
    //         Err(e) => {
    //             loggy.log(LogType::TextureError, &e.to_string());
    //         }
    //     }
    // }

    pub fn set_console_background_color(&mut self, r: u8, g: u8, b: u8, a: u8) {
        self.console_background_color = image::Rgba([r, g, b, a])
    }

    #[cfg(feature = "headed")]
    pub fn apply_console_out_text(&mut self) {
        let im = RgbaImage::new(self.size[0], self.size[1]);
        image::imageops::replace(&mut self.system_layer.image, &im, 0, 0);

        for (i, line) in self.console_string.lines().enumerate() {
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
                    &mut self.system_layer.image,
                    imageproc::rect::Rect::at((x) as i32, (y - 1) as i32)
                        .of_size(LETTER_SIZE + 1, LETTER_SIZE + 2 as u32),
                    self.console_background_color,
                );
                image::imageops::overlay(&mut self.system_layer.image, &mut sub.to_image(), x, y);
                x += (LETTER_SIZE - 1) as i64;
            }
        }

        // self.console =
        //     image::imageops::huerotate(&mut self.console, rand::thread_rng().gen_range(0..360));
        self.system_layer.dirty = true;
    }

    pub fn get_console_size(&self) -> (u32, u32) {
        (
            (self.size[0] / (LETTER_SIZE + 1) - 2),
            (self.size[1] / (LETTER_SIZE + 1) - 8),
        )
    }

    #[cfg(feature = "headed")]
    pub fn enable_console(&mut self, loggy: &Loggy) {
        self.console_string = loggy.get();
        self.output_console = true;
        self.apply_console_out_text();
    }
    #[cfg(feature = "headed")]
    pub fn disable_console(&mut self) {
        self.console_string = "".to_string();
        self.output_console = false;
        self.apply_console_out_text();
    }

    // pub fn overlay_image(&mut self, image: &RgbaImage) {
    //     image::imageops::overlay(&mut self.main, image, 0, 0);
    //     self.dirty = true;
    // }

    #[cfg(feature = "headed")]
    pub fn replace_image(&mut self, img: RgbaImage, index: ScreenIndex) {
        // println!("replacing image of size {:?}", img.dimensions());
        match index {
            ScreenIndex::System => {
                self.system_layer.image = img;
                self.system_layer.dirty = true;
            }
            ScreenIndex::Primary => {
                self.primary_layer.image = img;
                self.primary_layer.dirty = true;
            }
            ScreenIndex::Secondary => {
                self.secondary_layer.image = img;
                self.secondary_layer.dirty = true;
            }
            ScreenIndex::Trinary => {
                self.trinary_layer.image = img;
                self.trinary_layer.dirty = true;
            }
            ScreenIndex::Sky => {
                self.sky_layer.image = img;
                self.sky_layer.dirty = true;
            }
        }
    }

    #[cfg(feature = "headed")]
    pub fn resize(&mut self, size: (u32, u32), gfx: &crate::gfx::Gfx) {
        let device = &gfx.device;
        let queue = &gfx.queue;
        self.size = [size.0, size.1];
        // resize all layer fields
        self.system_layer.resize(device, queue, &self.size);
        self.primary_layer.resize(device, queue, &self.size);
        self.secondary_layer.resize(device, queue, &self.size);
        self.trinary_layer.resize(device, queue, &self.size);
        self.sky_layer.resize(device, queue, &self.size);

        let (gui_group, gui_aux_group, sky_group) = rebuild_group(
            [
                &self.system_layer.texture,
                &self.primary_layer.texture,
                &self.secondary_layer.texture,
                &self.trinary_layer.texture,
                &self.sky_layer.texture,
            ],
            device,
            &gfx.main_layout,
            &gfx.gui_aux_layout,
            &gfx.uniform_buf,
        );
        self.gui_group = gui_group;
        self.sky_group = sky_group;
        self.gui_aux_group = gui_aux_group;

        if self.output_console {
            self.apply_console_out_text();
        }
    }

    // pub fn change_layout(&mut self, gui_params: GuiParams) {
    //     self.local_gui_params = gui_params;
    // }

    #[cfg(feature = "headed")]
    pub fn render(&mut self, queue: &Queue, time: f32, loggy: &mut Loggy) {
        self.time = time;
        if loggy.is_dirty_and_listen() && self.output_console {
            self.console_string = loggy.get();
            self.apply_console_out_text();
            loggy.clean();
        }
        self.process_notifications(false);

        self.system_layer.check_render(queue);
        self.primary_layer.check_render(queue);
        self.secondary_layer.check_render(queue);
        self.trinary_layer.check_render(queue);
        self.sky_layer.check_render(queue);
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

    #[cfg(feature = "headed")]
    pub fn process_notifications(&mut self, force: bool) {
        let mut should = false;
        let mut i = 0;
        if self.notifcations_enabled {
            self.active_notifs.retain_mut(|n| {
                n.lifetime -= 1;
                if force || n.dest != n.position {
                    n.position -= (n.position - n.dest) / 20;

                    if !self.output_console {
                        if !should {
                            self.system_layer.image = RgbaImage::new(self.size[0], self.size[1]);
                            should = true;
                        }
                        // if n.image.is_none() {
                        //     n.render(&self.letters, self.size);
                        // }

                        // //guaranteed to be Some so unwrap ref
                        // if let Some(im) = &n.image {
                        //     image::imageops::replace(&mut self.notif, im, 0, n.position as i64);
                        //     // image::imageops::overlay(&mut self.notif, im, 0, n.position as i64);
                        // }

                        direct_text_raw(
                            &mut self.system_layer.image,
                            &self.letters,
                            &n.message,
                            0,
                            n.position,
                            if i % 2 == 0 {
                                vec4(1., 0., 1., 1.)
                            } else {
                                vec4(1., 0., 0., 1.)
                            },
                        );
                    }

                    // println!("notif pos {} dest {}", n.position, n.dest);
                } else {
                    // println!("still");
                }
                i += 1;
                n.lifetime > 0
            });
        }

        if should {
            self.system_layer.dirty = true;
            self.notifications_dirty = true;
            // self.main.dr(&self.notif, 0, 0);
        }
    }

    pub fn push_notif(&mut self, s: &str) {
        let mut n = Notif::new(s);
        n.position = -20;
        self.active_notifs.push(n);
        let len = self.active_notifs.len();
        // remove first element if there are more than 5
        if len > 5 {
            self.active_notifs.remove(0);
        }
        self.active_notifs
            .iter_mut()
            .enumerate()
            .for_each(|(i, no)| {
                no.dest = ((len - i) * 10) as i32;
            });
        // for (i, no) in self.active_notifs.iter_mut().enumerate() {
        //     if i > 2 {
        //         no.expired = true;
        //     }

        // }
        // if self.active_notifs.len() > 1 {
        //     self.queued_notifs.push(n);
        // } else {
        //     self.active_notifs.push(n);
        // }
    }
}

struct Notif {
    message: String,
    lifetime: u16,
    error: bool,
    dest: i32,
    position: i32,
    expired: bool,
    image: Option<RgbaImage>,
}

impl Notif {
    pub fn new(s: &str) -> Notif {
        Notif {
            message: s.to_string(),
            lifetime: 360,
            error: false,
            position: 0,
            dest: 0,
            expired: false,
            image: None,
        }
    }
    // pub fn render(&mut self, letters: &RgbaImage, size: [u32; 2]) {
    //     println!("rendering notif: {}", self.message);
    //     let mut img = RgbaImage::new(NOTIF_WIDTH, 64);
    //     let count = direct_text_raw(
    //         &mut img,
    //         &letters,
    //         size[0],
    //         size[1],
    //         &self.message,
    //         0,
    //         0,
    //         vec4(1., 1., 0., 1.),
    //     );
    //     crop_imm(&mut img, 0, 0, NOTIF_WIDTH, count * 16);
    //     self.image = Some(img);
    // }
}

pub type PreGuiMorsel = (RgbaImage, RgbaImage, RgbaImage, [u32; 2]);
pub struct GuiMorsel {
    pub letters: Rc<RgbaImage>,
    pub size: [u32; 2],
}

impl GuiMorsel {
    pub fn new(letters: RgbaImage, size: [u32; 2]) -> Self {
        // letters: Rc<RgbaImage>, main: RgbaImage, sky: RgbaImage, size: [u32; 2]
        let letters = Rc::new(letters);
        Self { letters, size }
    }

    // pub fn fill(&mut self, c: Vec4) {
    //     let width = self.size[0];
    //     let height = self.size[1];
    //     direct_fill(self.get_targ(), width, height, c);
    // }

    // pub fn rect(
    //     &mut self,
    //     x: LuaResponse,
    //     y: LuaResponse,
    //     w: LuaResponse,
    //     h: LuaResponse,
    //     c: Vec4,
    //     corner: Option<LuaResponse>,
    // ) {
    //     let width = self.size[0];
    //     let height = self.size[1];

    //     direct_rect(self.get_targ(), width, height, x, y, w, h, c, corner)
    // }

    // pub fn line(
    //     &mut self,
    //     x1: LuaResponse,
    //     y1: LuaResponse,
    //     x2: LuaResponse,
    //     y2: LuaResponse,
    //     c: Vec4,
    // ) {
    //     let width = self.size[0];
    //     let height = self.size[1];

    //     direct_line(self.get_targ(), width, height, x1, y1, x2, y2, c)
    // }

    // pub fn text(&mut self, txt: &str, x: LuaResponse, y: LuaResponse, c: Vec4) {
    //     let targ = if self.target_sky {
    //         self.dirty_sky = true;
    //         &mut self.sky
    //     } else {
    //         self.dirty = true;
    //         &mut self.main
    //     };

    //     direct_text(
    //         targ,
    //         &self.letters,
    //         self.size[0],
    //         self.size[1],
    //         txt,
    //         x,
    //         y,
    //         c,
    //     );
    // }

    // pub fn pixel(&mut self, x: u32, y: u32, rgb: Vec4) {
    //     let s = self.size;
    //     println!("morself gui {} {}", s[0], s[1]);

    //     direct_pixel(self.get_targ(), s[0], s[1], x, y, rgb.to_array());

    // }
    pub fn resize(&mut self, w: u32, h: u32) {
        self.size = [w, h];
        println!("morself gui {} {}", w, h);
        // self.main = image::imageops::resize(&self.main, w, h, image::imageops::FilterType::Nearest);
        // self.sky = image::imageops::resize(&self.sky, w, h, image::imageops::FilterType::Nearest);
        // self.dirty = true;
        // self.dirty_sky = true;
    }

    // pub fn draw_image(&mut self, image: &RgbaImage, x: LuaResponse, y: LuaResponse) {
    //     let xx = eval(x, self.size[0]);
    //     let yy = eval(y, self.size[1]);
    //     image::imageops::overlay(self.get_targ(), image, xx as i64, yy as i64);
    // }

    /* Clean off the main raster */
    // pub fn clean(&mut self) {
    //     let sx = self.size[0];
    //     let sy = self.size[1];
    //     // clear image to transparent
    //     self.get_targ().copy_from(&RgbaImage::new(sx, sy), 0, 0);
    // }

    // pub fn target_gui(&mut self) {
    //     self.target_sky = false;
    // }
    // pub fn target_sky(&mut self) {
    //     self.target_sky = true;
    // }

    // fn get_targ(&mut self) -> &mut RgbaImage {
    //     if self.target_sky {
    //         self.dirty_sky = true;
    //         &mut self.sky
    //     } else {
    //         self.dirty = true;
    //         &mut self.main
    //     }
    // }

    pub fn new_image(w: u32, h: u32) -> RgbaImage {
        RgbaImage::new(w, h)
    }
    //     pub fn send_state(&mut self) -> (Option<RgbaImage>, Option<RgbaImage>) {
    //         if self.dirty {
    //             self.dirty = false;
    //             if self.dirty_sky {
    //                 self.dirty_sky = false;
    //                 (Some(self.main.clone()), Some(self.sky.clone()))
    //             } else {
    //                 (Some(self.main.clone()), None)
    //             }
    //         } else if self.dirty_sky {
    //             self.dirty_sky = false;
    //             (None, Some(self.sky.clone()))
    //         } else {
    //             (None, None)
    //         }
    //     }
}

pub fn eval(val: LuaResponse, l: u32) -> i32 {
    match val {
        LuaResponse::Integer(i) => {
            if i < 0 {
                // l as i32 - i
                i
            } else {
                i
            }
        }
        LuaResponse::Number(f) => {
            let ff = (f * l as f64) as i32;
            if f < 0. {
                // l as i32 - ff
                ff
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

#[cfg(feature = "headed")]
pub fn rebuild_group(
    gui_bundles: [&TexTuple; 5],
    device: &Device,
    main_layout: &wgpu::BindGroupLayout,
    gui_aux_layout: &wgpu::BindGroupLayout,
    uniform_buf: &wgpu::Buffer,
) -> (BindGroup, BindGroup, BindGroup) {
    let gui_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &main_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&gui_bundles[0].view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&gui_bundles[0].sampler),
            },
        ],
        label: None,
    });
    let gui_aux_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &gui_aux_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&gui_bundles[1].view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&gui_bundles[2].view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::TextureView(&gui_bundles[3].view),
            },
        ],
        label: None,
    });

    let sky_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        layout: &main_layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(&gui_bundles[4].view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(&gui_bundles[4].sampler),
            },
        ],
        label: None,
    });
    (gui_group, gui_aux_group, sky_group)
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

#[cfg(feature = "headed")]
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

/** evaluate position with GuiUnits then draw letters over image. Returns the line count */
pub fn direct_text(
    target: &mut RgbaImage,
    letters: &Rc<RgbaImage>,
    width: u32,
    height: u32,
    txt: &str,
    x: LuaResponse,
    y: LuaResponse,
    c: Vec4,
) -> u32 {
    let xx = eval(x, width);
    let yy = eval(y, height);
    direct_text_raw(target, letters.borrow(), txt, xx, yy, c)
}

/** Even rawer! Just hand us the i32 positions. Returns the line count */
pub fn direct_text_raw(
    target: &mut RgbaImage,
    letters: &RgbaImage,
    txt: &str,
    xx: i32,
    yy: i32,
    color: Vec4,
) -> u32 {
    let map = TextMap {
        new_color: image::Rgba([
            (color.x * 255.) as u8,
            (color.y * 255.) as u8,
            (color.z * 255.) as u8,
            (color.w * 255.) as u8,
        ]),
    };
    let mut count: u32 = 0;
    for (i, line) in txt.lines().enumerate() {
        let ly = yy + i as i32 * (LETTER_SIZE as i32 + 2);
        let mut lx = xx + (0) as i32;
        for c in line.chars() {
            let mut ind = c as u32;
            if ind > 255 {
                ind = 255;
            }
            let index_x = ind % 16;
            let index_y = ind / 16;
            let sub: image::SubImage<&RgbaImage> = image::imageops::crop_imm(
                letters,
                index_x * LETTER_SIZE,
                index_y * LETTER_SIZE,
                LETTER_SIZE,
                LETTER_SIZE,
            );
            // change color of sub to red
            let mut im = sub.to_image();
            // image::imageops::colorops::index_colors(image, color_map)
            // subi.pixels().for_each(|p| {
            //     let mut p = p.0;
            //     p[0] = (c.x * 255.) as u8;
            //     p[1] = (c.y * 255.) as u8;
            //     p[2] = (c.z * 255.) as u8;
            //     p[3] = (c.w * 255.) as u8;
            // });
            // image::imageops::colorops::rep lace(&mut subi, c.into());
            // let mut indices: RgbaImage = ImageBuffer::new(im.width(), im.height());
            for pixel in im.pixels_mut() {
                map.map_color(pixel);
                //*idx = Luma(
            }

            // let r: RgbaImage = image::imageops::colorops::index_colors(&mut subi, &map);

            image::imageops::overlay(target, &im, lx.into(), ly.into());
            lx += (LETTER_SIZE + 0) as i32;
        }
        count += 1;
    }
    count
}

/** Directly draw pixel to image position */
pub fn direct_pixel(
    target: &mut RgbaImage,
    x: u32,
    y: u32,
    iwidth: u32,
    iheight: u32,
    rgb: [f32; 4],
) {
    target
        .get_pixel_mut(x.min(iwidth - 1), y.min(iheight - 1))
        .0 = [
        (rgb[0] * 255.) as u8,
        (rgb[1] * 255.) as u8,
        (rgb[2] * 255.) as u8,
        (rgb[3] * 255.) as u8,
    ];
}

/** Directly draw image to image using GuiUnits for position. Uses source image dimensions */
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

pub fn direct_fill(target: &mut RgbaImage, width: u32, height: u32, c: Vec4, map: Option<Vec4>) {
    let color = image::Rgba([
        (c.x * 255.).floor() as u8,
        (c.y * 255.).floor() as u8,
        (c.z * 255.).floor() as u8,
        (c.w * 255.).floor() as u8,
    ]);
    //     pixel
    // let pix=Pixel::from(color);
    match map {
        Some(mv) => {
            let mapper = image::Rgba([
                (mv.x * 255.).floor() as u8,
                (mv.y * 255.).floor() as u8,
                (mv.z * 255.).floor() as u8,
                (mv.w * 255.).floor() as u8,
            ]);
            // println!("map {:?} to {:?}", mapper, mv);
            imageproc::map::map_pixels_mut(target, |x, y, p| {
                if mapper == p {
                    color
                } else {
                    // println!("ignored {:?} ", p);
                    p
                }
            });
        }
        None => {
            draw_filled_rect_mut(
                target,
                imageproc::rect::Rect::at(0, 0).of_size(width, height),
                color,
            );
        }
    }
}

struct TextMap {
    new_color: Rgba<u8>,
}
impl ColorMap for TextMap {
    fn index_of(&self, color: &Self::Color) -> usize {
        // color.
        todo!()
    }

    fn map_color(&self, color: &mut Self::Color) {
        if color.0[3] > 0 {
            *color = self.new_color;
        }
    }

    type Color = Rgba<u8>;
}

/** Mutate the image provided to the dimensions */
pub fn resizer(im: &mut RgbaImage, w: u32, h: u32) {
    *im = image::imageops::resize(im, w, h, image::imageops::FilterType::Nearest);
}

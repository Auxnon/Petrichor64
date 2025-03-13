use std::{ops::Deref, rc::Rc};

use crate::{
    command::{num, numop},
    gui::{direct_fill, direct_image, direct_line, direct_pixel, direct_rect, direct_text},
    userdata_util::StaticUserMethods,
};
use glam::{vec4, Vec4};
use image::RgbaImage;
#[cfg(feature = "puc_lua")]
use mlua::{AnyUserData, UserData, UserDataMethods, Value};
#[cfg(feature = "picc")]
use piccolo::{AnyUserData, Value};
use piccolo::{Context, Lua};

#[cfg(feature = "silt")]
use silt_lua::prelude::{UserData, Value};

pub struct LuaImg {
    pub dirty: bool,
    pub bundle_id: u8,
    pub width: u32,
    pub height: u32,
    pub image: RgbaImage,
    letters: Rc<RgbaImage>,
}

impl LuaImg {
    pub fn new(
        bundle_id: u8,
        image: RgbaImage,
        width: u32,
        height: u32,
        letters: Rc<RgbaImage>,
    ) -> Self {
        Self {
            dirty: true,
            bundle_id,
            image,
            width,
            height,
            letters,
        }
    }
    pub fn empty() -> Self {
        Self {
            dirty: false,
            bundle_id: 0,
            image: RgbaImage::new(1, 1),
            width: 1,
            height: 1,
            letters: Rc::new(RgbaImage::new(1, 1)),
        }
    }
    pub fn clone(&self) -> Self {
        Self {
            dirty: self.dirty,
            bundle_id: self.bundle_id,
            image: self.image.clone(),
            width: self.width,
            height: self.height,
            letters: self.letters.clone(),
        }
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        crate::gui::resizer(&mut self.image, width, height);
        self.dirty = true;
    }
}

#[cfg(feature = "puc_lua")]
impl UserData for LuaImg {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        // methods.add_method_mut(name, method)
        methods.add_method("raw", |_, this, _: ()| Ok(this.image.to_vec()));
        methods.add_method_mut(
            "line",
            |_, this, (x, y, x2, y2, rgb): (Value, Value, Value, Value, Value)| {
                this.dirty = true;
                let c = get_color(rgb);
                direct_line(
                    &mut this.image,
                    this.width,
                    this.height,
                    num(x),
                    num(y),
                    num(x2),
                    num(y2),
                    c,
                );
                Ok(())
            },
        );

        methods.add_method_mut(
            "rect",
            |_, this, (x, y, w, h, rgb): (Value, Value, Value, Value, Value)| {
                this.dirty = true;
                let c = get_color(rgb);
                direct_rect(
                    &mut this.image,
                    this.width,
                    this.height,
                    num(x),
                    num(y),
                    num(w),
                    num(h),
                    c,
                    None,
                );
                Ok(())
            },
        );
        methods.add_method_mut(
            "rrect",
            |_, this, (x, y, w, h, ro, rgb): (Value, Value, Value, Value, Value, Value)| {
                this.dirty = true;
                let c = get_color(rgb);
                direct_rect(
                    &mut this.image,
                    this.width,
                    this.height,
                    num(x),
                    num(y),
                    num(w),
                    num(h),
                    c,
                    Some(num(ro)),
                );
                Ok(())
            },
        );

        methods.add_method_mut(
            "text",
            |_, this, (txt, x, y, rgb): (String, Option<Value>, Option<Value>, Option<Value>)| {
                this.dirty = true;
                let c = match rgb {
                    Some(rgba) => get_color(rgba),
                    _ => vec4(1., 1., 1., 1.),
                };
                direct_text(
                    &mut this.image,
                    &this.letters,
                    this.width,
                    this.height,
                    &txt,
                    numop(x),
                    numop(y),
                    c,
                );
                Ok(())
            },
        );
        methods.add_method_mut(
            "img",
            |_, this, (img, x, y): (AnyUserData, Option<Value>, Option<Value>)| {
                this.dirty = true;
                if let Ok(limg) = img.borrow::<LuaImg>() {
                    direct_image(
                        &mut this.image,
                        &limg.image,
                        numop(x),
                        numop(y),
                        this.width,
                        this.height,
                    );
                }
                Ok(())
            },
        );
        methods.add_method_mut(
            "pixel",
            |_, this, (x, y, rgb): (u32, u32, Option<Value>)| {
                this.dirty = true;
                let c = match rgb {
                    Some(rgba) => get_color(rgba),
                    _ => vec4(1., 1., 1., 1.),
                };

                direct_pixel(&mut this.image, x, y, this.width, this.height, c.to_array());

                Ok(())
            },
        );
        methods.add_method_mut("clr", |_, this, (): ()| {
            this.dirty = true;
            this.image = RgbaImage::new(this.width, this.height);
            Ok(())
        });
        methods.add_method_mut(
            "fill",
            |_, this, (rgb, map): (Option<Value>, Option<Value>)| {
                this.dirty = true;
                let c = match rgb {
                    Some(rgba) => get_color(rgba),
                    _ => vec4(1., 1., 1., 1.),
                };
                let mapper = match map {
                    Some(m) => Some(get_color(m)),
                    _ => None,
                };

                direct_fill(&mut this.image, this.width, this.height, c, mapper);
                // this.image = RgbaImage::new(this.width, this.height);
                Ok(())
            },
        );
        methods.add_method("copy", |_, this, (): ()| Ok(this.clone()));

        // TODO from raw
        // if let Ok(img) = im.get::<_, Vec<u8>>("data") {
        //     if let Ok(w) = im.get::<_, u32>("w") {
        //         if let Ok(h) = im.get::<_, u32>("h") {
        //             let len = img.len();
        //             if let Some(rgba) = RgbaImage::from_raw(w, h, img) {
        //                 // println!("got image {}x{} w len {}", w, h, len);
        //                 gui.borrow_mut().draw_image(
        //                     &rgba,
        //                     match x {
        //                         Some(o) => numm(o),
        //                         _ => (false, 0.),
        //                     },
        //                     match y {
        //                         Some(o) => numm(o),
        //                         _ => (false, 0.),
        //                     },
        //                 );
        //             }
        //         }
        //     }
        // }
    }
}

pub fn lua_img_constructor<'gc>(ctx: &Context<'gc>, limg: LuaImg) -> AnyUserData<'gc> {
    // let mc

    let methods = StaticUserMethods::<LuaImg>::new(ctx.deref());
    methods.add("raw", *ctx, |this, x, fuel, _: ()| Ok(this.image.to_vec()));
    let ud = methods.wrap(*ctx, limg);
    return ud;
}

pub fn dehex(s2: &str) -> Vec4 {
    let s = if s2.starts_with("#") {
        &s2[1..s2.len()]
    } else {
        s2
    };

    let halfed = s2.len() < 4;
    let res = if halfed {
        half_decode_hex(s)
    } else {
        decode_hex(s)
    };

    match res {
        Ok(b) => {
            if b.len() > 2 {
                let f = b
                    .iter()
                    .map(|u| (*u as f32) / if halfed { 15. } else { 255. })
                    .collect::<Vec<f32>>();
                vec4(f[0], f[1], f[2], if b.len() > 3 { f[3] } else { 1. })
            } else {
                vec4(0., 0., 0., 0.)
            }
        }
        _ => vec4(0., 0., 0., 0.),
    }
}

pub fn get_color<'gc>(ctx: &Context<'gc>, x: Value<'gc>) -> Vec4 {
    match x {
        Value::String(s) => match s.to_str() {
            Ok(s2) => dehex(s2),
            _ => vec4(0., 0., 0., 0.),
        },
        Value::Table(t) => {
            // let tt = t.next(key)
            //     .sequence_values::<f32>()
            //     .filter_map(|f| match f {
            //         Ok(v) => Some(v),
            //         _ => None,
            //     })
            //     .collect::<Vec<f32>>();
            let c = *ctx;

            let mut r = t.get(c, 0).to_number().unwrap_or(0.);
            let g = t.get(c, 1).to_number().unwrap_or(0.);
            let b = t.get(c, 2).to_number().unwrap_or(0.);
            let a = t.get(c, 3).to_number().unwrap_or(0.);
            if r > 1. {
                r = r / 255.;
            }
            if g > 1. {
                r = g / 255.;
            }
            if b > 1. {
                r = b / 255.;
            }
            if a > 1. {
                r = a / 255.;
            }

            vec4(r as f32, g as f32, b as f32, a as f32)
        }
        _ => vec4(1., 1., 1., 1.),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, core::num::ParseIntError> {
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16))
        .collect()
}

fn half_decode_hex(s: &str) -> Result<Vec<u8>, core::num::ParseIntError> {
    (0..s.len())
        .map(|i| u8::from_str_radix(&s[i..i + 1], 16))
        .collect()
}

impl ToString for LuaImg {
    fn to_string(&self) -> String {
        format!("image({}x{})", self.width, self.height)
    }
}

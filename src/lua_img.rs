use std::rc::Rc;

use glam::{vec4, Vec4};
use image::RgbaImage;
use mlua::{AnyUserData, UserData, UserDataMethods, Value};

use crate::{
    command::NumCouple,
    gui::{direct_image, direct_line, direct_rect, direct_text},
};

pub struct LuaImg {
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
            bundle_id,
            image,
            width,
            height,
            letters,
        }
    }
    pub fn empty() -> Self {
        Self {
            bundle_id: 0,
            image: RgbaImage::new(1, 1),
            width: 1,
            height: 1,
            letters: Rc::new(RgbaImage::new(1, 1)),
        }
    }
    pub fn clone(&self) -> Self {
        Self {
            bundle_id: self.bundle_id,
            image: self.image.clone(),
            width: self.width,
            height: self.height,
            letters: self.letters.clone(),
        }
    }
}

impl UserData for LuaImg {
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        // methods.add_method_mut(name, method)
        methods.add_method("raw", |_, this, _: ()| Ok(this.image.to_vec()));
        methods.add_method_mut(
            "line",
            |_,
             this,
             (x, y, x2, y2, r, g, b, a): (
                Value,
                Value,
                Value,
                Value,
                Value,
                Option<f32>,
                Option<f32>,
                Option<f32>,
            )| {
                let c = get_color(r, g, b, a);
                direct_line(
                    &mut this.image,
                    this.width,
                    this.height,
                    numm(x),
                    numm(y),
                    numm(x2),
                    numm(y2),
                    c,
                );
                Ok(())
            },
        );

        methods.add_method_mut(
            "rect",
            |_,
             this,
             (x, y, w, h, r, g, b, a): (
                Value,
                Value,
                Value,
                Value,
                Value,
                Option<f32>,
                Option<f32>,
                Option<f32>,
            )| {
                let c = get_color(r, g, b, a);
                direct_rect(
                    &mut this.image,
                    this.width,
                    this.height,
                    numm(x),
                    numm(y),
                    numm(w),
                    numm(h),
                    c,
                );
                Ok(())
            },
        );

        methods.add_method_mut(
            "text",
            |_,
             this,
             (txt, x, y, r, g, b, a): (
                String,
                Option<Value>,
                Option<Value>,
                Option<Value>,
                Option<f32>,
                Option<f32>,
                Option<f32>,
            )| {
                let c = match r {
                    Some(rr) => get_color(rr, g, b, a),
                    _ => vec4(1., 1., 1., 1.),
                };
                direct_text(
                    &mut this.image,
                    &this.letters,
                    this.width,
                    this.height,
                    &txt,
                    match x {
                        Some(o) => numm(o),
                        _ => (false, 0.),
                    },
                    match y {
                        Some(o) => numm(o),
                        _ => (false, 0.),
                    },
                    c,
                );
                Ok(())
            },
        );
        methods.add_method_mut(
            "dimg",
            |_, this, (img, x, y): (AnyUserData, Option<Value>, Option<Value>)| {
                if let Ok(limg) = img.borrow::<LuaImg>() {
                    direct_image(
                        &mut this.image,
                        &limg.image,
                        match x {
                            Some(o) => numm(o),
                            _ => (false, 0.),
                        },
                        match y {
                            Some(o) => numm(o),
                            _ => (false, 0.),
                        },
                        this.width,
                        this.height,
                    );
                }
                Ok(())
            },
        );
        methods.add_method_mut("clr", |_, this, (): ()| {
            this.image = RgbaImage::new(this.width, this.height);
            Ok(())
        });
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

fn numm(x: mlua::Value) -> NumCouple {
    match x {
        mlua::Value::Integer(i) => (true, i as f32),
        mlua::Value::Number(f) => (false, f as f32),
        _ => (false, 0.),
    }
}

pub fn get_color(x: mlua::Value, y: Option<f32>, z: Option<f32>, w: Option<f32>) -> Vec4 {
    match x {
        mlua::Value::String(s) => match s.to_str() {
            Ok(s2) => {
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
            _ => vec4(0., 0., 0., 0.),
        },
        mlua::Value::Integer(i) => vec4(
            i as f32,
            y.unwrap_or_else(|| 0.),
            z.unwrap_or_else(|| 0.),
            w.unwrap_or_else(|| 1.),
        ),
        mlua::Value::Number(f) => vec4(
            f as f32,
            y.unwrap_or_else(|| 0.),
            z.unwrap_or_else(|| 0.),
            w.unwrap_or_else(|| 1.),
        ),
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

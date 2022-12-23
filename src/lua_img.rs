use std::rc::Rc;

use image::RgbaImage;
use mlua::{AnyUserData, UserData, UserDataMethods, Value};

use crate::{
    command::NumCouple,
    gui::{direct_image, direct_line, direct_text},
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
            |_, this, (x, y, x2, y2, color): (Value, Value, Value, Value, Option<String>)| {
                direct_line(
                    &mut this.image,
                    this.width,
                    this.height,
                    numm(x),
                    numm(y),
                    numm(x2),
                    numm(y2),
                );
                Ok(())
            },
        );

        methods.add_method_mut(
            "text",
            |_, this, (txt, x, y): (String, Option<Value>, Option<Value>)| {
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

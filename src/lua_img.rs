use std::sync::Arc;

use crate::{
    command::{num, numop},
    gui::{direct_fill, direct_image, direct_line, direct_pixel, direct_rect, direct_text},
};
use glam::{vec4, Vec4};
use image::RgbaImage;
#[cfg(feature = "puc_lua")]
use mlua::{AnyUserData, UserData, UserDataMethods, Value};
#[cfg(feature = "picc")]
use piccolo::{AnyUserData, Value};

#[cfg(feature = "silt")]
use silt_lua::prelude::{UserData, Value};
use silt_lua::{
    userdata::{UserDataFields, UserDataMethods},
    LuaError,
};

pub struct LuaImg {
    pub dirty: bool,
    pub bundle_id: u8,
    pub width: u32,
    pub height: u32,
    pub image: RgbaImage,
    /// The last *completed* frame, and whether it still needs uploading.
    ///
    /// `image` is written a draw call at a time by the Lua thread, so reading it
    /// from the renderer catches half-drawn frames — after a `clr()` and before
    /// anything is drawn back, the screen flashes empty. `publish` copies a whole
    /// frame here at the end of a loop, under the same lock the renderer takes, so
    /// the renderer only ever sees frames that were finished.
    ///
    /// `None` until something publishes: only the gui and sky rasters are ever
    /// presented, and an image the game made with `nimg` shouldn't pay for a second
    /// buffer it never uses.
    presented: Option<RgbaImage>,
    presented_dirty: bool,
    letters: Arc<RgbaImage>,
}

impl LuaImg {
    pub fn new(
        bundle_id: u8,
        image: RgbaImage,
        width: u32,
        height: u32,
        letters: Arc<RgbaImage>,
    ) -> Self {
        Self {
            dirty: true,
            bundle_id,
            image,
            width,
            height,
            presented: None,
            presented_dirty: false,
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
            presented: None,
            presented_dirty: false,
            letters: Arc::new(RgbaImage::new(1, 1)),
        }
    }
    pub fn clone(&self) -> Self {
        Self {
            dirty: self.dirty,
            bundle_id: self.bundle_id,
            image: self.image.clone(),
            width: self.width,
            height: self.height,
            presented: None,
            presented_dirty: false,
            letters: self.letters.clone(),
        }
    }
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        crate::gui::resizer(&mut self.image, width, height);
        // Drop the presented copy rather than resize it: it belongs to the old
        // dimensions, and uploading it against the new texture would be an
        // out-of-bounds copy. The next publish rebuilds it at the right size.
        self.presented = None;
        self.presented_dirty = false;
        self.dirty = true;
    }

    /// Called by the owning Lua thread once its loop has finished drawing: take a
    /// copy of the completed frame for the renderer. Returns whether there was
    /// anything new to publish.
    pub fn publish(&mut self) -> bool {
        if !self.dirty {
            return false;
        }
        match self.presented.as_mut() {
            Some(p) if p.dimensions() == self.image.dimensions() => {
                p.copy_from_slice(&self.image);
            }
            _ => self.presented = Some(self.image.clone()),
        }
        self.dirty = false;
        self.presented_dirty = true;
        true
    }

    /// Renderer side: hand over the completed frame if one is waiting.
    pub fn take_presented(&mut self) -> Option<&RgbaImage> {
        if !self.presented_dirty {
            return None;
        }
        self.presented_dirty = false;
        self.presented.as_ref()
    }
}

macro_rules! safe_unwrap {
    ($ud:ident) => {
        if let Some(ud) = $ud {
            ud
        } else {
            return Err(LuaError::UDBadCast);
        }
    };
}

// #[cfg(feature = "puc_lua")]
impl UserData for LuaImg {
    fn type_name() -> &'static str {
        "Ent"
    }
    fn get_id(&self) -> usize {
        2
    }
    /// Read-only pixel size, so scripts can lay out text and project to `gui`.
    fn add_fields<'lua, F: UserDataFields<'lua, Self>>(fields: &mut F) {
        fields.add_field_method_get("w", |_, _, this| this.width as i64);
        fields.add_field_method_get("h", |_, _, this| this.height as i64);
    }
    fn add_methods<'lua, M: UserDataMethods<'lua, Self>>(methods: &mut M) {
        // methods.add_method_mut(name, method)
        methods.add_method("raw", |vm, mc, this, _: ()| {
            let im = safe_unwrap!(this);
            Ok(im.image.to_vec())
            // let res = <Vec<u8> as ToLua>::to_lua(v, vm, mc);
            // let res=ToLua::
            //
            // Ok(Value::Nil)
        });

        methods.add_method_mut(
            "line",
            |_vm, _mc, this, (x, y, x2, y2, rgb): (Value, Value, Value, Value, Value)| {
                let im = safe_unwrap!(this);
                im.dirty = true;
                let c = get_color(rgb);
                direct_line(
                    &mut im.image,
                    im.width,
                    im.height,
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
            |_, _mc, this, (x, y, w, h, rgb): (Value, Value, Value, Value, Value)| {
                let im = safe_unwrap!(this);
                im.dirty = true;
                let c = get_color(rgb);
                direct_rect(
                    &mut im.image,
                    im.width,
                    im.height,
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
            |_, _, this_res, (x, y, w, h, ro, rgb): (Value, Value, Value, Value, Value, Value)| {
                let this = safe_unwrap!(this_res);
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
            |_,
             _,
             this_res,
             (txt, x, y, rgb): (String, Option<Value>, Option<Value>, Option<Value>)| {
                let this = safe_unwrap!(this_res);
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
            |_,
             _,
             this_res,
             (img, x, y, scale): (Value, Option<Value>, Option<Value>, Option<Value>)| {
                let this = safe_unwrap!(this_res);
                this.dirty = true;
                // Optional whole-number zoom, nearest-neighbour: a pixel-art
                // canvas view without drawing every texel as its own rect.
                let scale = match scale {
                    Some(Value::Integer(i)) if i > 1 => i as u32,
                    Some(Value::Number(f)) if f >= 2. => f as u32,
                    _ => 1,
                };
                img.apply_userdata(|limg: &LuaImg| {
                    if scale > 1 {
                        let big = image::imageops::resize(
                            &limg.image,
                            limg.image.width() * scale,
                            limg.image.height() * scale,
                            image::imageops::FilterType::Nearest,
                        );
                        direct_image(&mut this.image, &big, numop(x), numop(y), this.width, this.height);
                    } else {
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
                });
                Ok(())
            },
        );

        // Read one pixel back, as an `rrggbbaa` string any colour argument
        // takes; nil outside the image.
        methods.add_method("get", |_, _mc, this_res, (x, y): (i64, i64)| {
            let this = safe_unwrap!(this_res);
            if x < 0 || y < 0 || x >= this.image.width() as i64 || y >= this.image.height() as i64 {
                return Ok(None);
            }
            let p = this.image.get_pixel(x as u32, y as u32).0;
            Ok(Some(crate::command::hex_rgba(p)))
        });

        methods.add_method_mut(
            "pixel",
            |_, _mc, this_res, (x, y, rgb): (u32, u32, Option<Value>)| {
                let this = safe_unwrap!(this_res);
                this.dirty = true;
                let c = match rgb {
                    Some(rgba) => get_color(rgba),
                    _ => vec4(1., 1., 1., 1.),
                };

                direct_pixel(&mut this.image, x, y, this.width, this.height, c.to_array());
                Ok(())
            },
        );

        methods.add_method_mut("clr", |_, _mc, this_res, (): ()| {
            let this = safe_unwrap!(this_res);
            this.dirty = true;
            this.image = RgbaImage::new(this.width, this.height);
            Ok(())
        });

        methods.add_method_mut(
            "fill",
            |_, _, this_res, (rgb, map): (Option<Value>, Option<Value>)| {
                let this = safe_unwrap!(this_res);
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

        methods.add_method_mut("copy", |vm, mc, this_res, (): ()| {
            let this = safe_unwrap!(this_res);
            // Returned: this used to build the copy and drop it, so `copy()`
            // always gave back nil.
            Ok(vm.create_userdata(mc, this.clone()))
        });

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

// pub fn lua_img_constructor<'gc>(ctx: &Context<'gc>, limg: LuaImg) -> AnyUserData<'gc> {
//     // let mc
//
//     let methods = StaticUserMethods::<LuaImg>::new(ctx.deref());
//     methods.add("raw", *ctx, |this, x, fuel, _: ()| Ok(this.image.to_vec()));
//     let ud = methods.wrap(*ctx, limg);
//     return ud;
// }

pub fn dehex(s2: &str) -> Vec4 {
    let s = if s2.starts_with("#") {
        &s2[1..s2.len()]
    } else {
        s2
    };

    // RGB / RGBA are one digit a channel, RRGGBB / RRGGBBAA two. Measured
    // without the '#'. It was `s2.len() < 4`: 4-digit RGBA ("0009") was read
    // as two 2-digit pairs, too few channels, so came out fully transparent
    // (and drew as an eraser), and "#FFF" sliced past the end and panicked.
    let halfed = s.len() <= 4;
    let res = if halfed {
        half_decode_hex(s)
    } else if s.len() % 2 == 0 {
        decode_hex(s)
    } else {
        // odd length past 4: not a colour (and would slice past the end)
        return vec4(0., 0., 0., 0.);
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

pub fn get_color<'gc>(x: Value<'gc>) -> Vec4 {
    match x {
        Value::String(s) => dehex(&s),
        Value::Table(tab) => {
            // `{r, g, b, a?}`, 1-based like any Lua array (see guide/color.md).
            // If any channel is over 1 the whole table is 0-255, otherwise 0-1, so
            // a hand-typed `{255, 1, 0}` keeps its 1 dark and `{1, 1, 1}` is still
            // white. Missing rgb is 0, missing alpha opaque.
            let t = tab.borrow();
            let mut vals = [None::<f32>; 4];
            for i in 0..4 {
                vals[i] = match t.getn(i + 1) {
                    Some(Value::Integer(n)) => Some(*n as f32),
                    Some(Value::Number(n)) => Some(*n as f32),
                    _ => None,
                };
            }
            let over_one = vals.iter().flatten().any(|v| *v > 1.);
            let scale = if over_one { 255. } else { 1. };
            let c = |i: usize, missing: f32| vals[i].map(|v| v / scale).unwrap_or(missing);
            vec4(c(0, 0.), c(1, 0.), c(2, 0.), c(3, 1.))
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

#[cfg(test)]
mod hex_tests {
    use super::dehex;
    use glam::vec4;

    #[test]
    fn every_hex_form_parses() {
        assert_eq!(dehex("F00"), vec4(1., 0., 0., 1.));
        assert_eq!(dehex("#F00"), vec4(1., 0., 0., 1.));
        // RGBA short: was fully transparent
        assert_eq!(dehex("0009"), vec4(0., 0., 0., 9. / 15.));
        assert_eq!(dehex("#0009"), vec4(0., 0., 0., 9. / 15.));
        assert_eq!(dehex("FF0000"), vec4(1., 0., 0., 1.));
        assert_eq!(dehex("#00FF0080"), vec4(0., 1., 0., 128. / 255.));
        // nonsense is transparent, never a panic
        assert_eq!(dehex("12345"), vec4(0., 0., 0., 0.));
        assert_eq!(dehex("zz"), vec4(0., 0., 0., 0.));
    }
}

//! The gear and the command strip: the studio's way in without a keyboard.
//!
//! A small gear sits in the bottom right corner. Pressing it opens the console
//! and a strip of tool icons along the bottom of the screen, resting on the
//! soft keyboard when one is docked there. Pressing a tool closes the console
//! and the strip and opens that tool's overlay. The backtick key does the same
//! as the gear.
//!
//! This module is only layout, hit testing and pixels, in gui-layer pixels, so
//! it can be tested without an engine. `Core` (root.rs) owns the state and
//! acts on what was pressed.

use image::{Rgba, RgbaImage};

/// Everything that can be pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Gear,
    Paint,
    Model,
    Edit,
    Sound,
    Share,
    /// Share's two choices, and the way back to the tools.
    Png,
    Zip,
    Back,
}

/// The tools, in the order they sit on the strip.
pub const TOOLS: [Item; 5] = [
    Item::Paint,
    Item::Model,
    Item::Edit,
    Item::Sound,
    Item::Share,
];
const SHARE: [Item; 3] = [Item::Png, Item::Zip, Item::Back];

pub const ICON: i32 = 8;
/// The smallest cell: an icon and a 2 pixel margin.
pub const MIN_CELL: u32 = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn has(&self, x: i32, y: i32) -> bool {
        x >= self.x && y >= self.y && x < self.x + self.w && y < self.y + self.h
    }
}

/// What the layout depends on. Two equal frames draw the same pixels, which is
/// how the gui knows when to repaint.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Frame {
    /// The gui layer's size.
    pub gui: (u32, u32),
    /// The side of one pressable cell: at least `MIN_CELL`, larger where a
    /// finger has to hit it.
    pub cell: u32,
    /// How much of the bottom a docked soft keyboard covers. 0 when there is
    /// none, or it floats.
    pub keyboard: u32,
    /// The system's bars at the bottom and right edges (a phone's navigation
    /// bar), which the gear stays clear of.
    pub bottom: u32,
    pub right: u32,
    /// Tools against the gear rather than centred: portrait screens and
    /// tablets.
    pub right_justified: bool,
    /// The strip is out (the console is open).
    pub open: bool,
    /// Showing share's choices instead of the tools.
    pub share: bool,
    /// The tool whose overlay is up, drawn lit.
    pub active: Option<Item>,
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Layout {
    /// The strip's background, when it is out.
    pub bar: Option<Rect>,
    /// Every pressable cell, the gear included.
    pub cells: Vec<(Item, Rect)>,
}

pub fn layout(f: &Frame) -> Layout {
    let (w, h) = (f.gui.0 as i32, f.gui.1 as i32);
    let cell = (f.cell.max(MIN_CELL) as i32).min(w.max(1)).min(h.max(1));
    // A docked keyboard already sits above the navigation bar.
    let lift = f.keyboard.max(f.bottom) as i32;
    let y = (h - lift - cell).max(0);
    let gear = Rect {
        x: (w - f.right as i32 - cell).max(0),
        y,
        w: cell,
        h: cell,
    };
    let mut out = Layout {
        bar: None,
        cells: vec![(Item::Gear, gear)],
    };
    if !f.open {
        return out;
    }
    out.bar = Some(Rect {
        x: 0,
        y,
        w,
        h: cell,
    });
    let items: &[Item] = if f.share { &SHARE } else { &TOOLS };
    let run = cell * items.len() as i32;
    // Never under the gear, and never off the left edge.
    let most = (gear.x - run).max(0);
    let x0 = if f.right_justified {
        most
    } else {
        ((w - run) / 2).clamp(0, most)
    };
    for (i, item) in items.iter().enumerate() {
        let x = x0 + cell * i as i32;
        // A screen too narrow for them all shows the ones that fit.
        if x + cell > gear.x {
            break;
        }
        out.cells.push((
            *item,
            Rect {
                x,
                y,
                w: cell,
                h: cell,
            },
        ));
    }
    out
}

/// What a press at a gui pixel lands on. `None` with `covers` true is the
/// strip's own background: swallowed, but nothing happens.
pub fn hit(l: &Layout, x: i32, y: i32) -> Option<Item> {
    // Tools first: on a very narrow screen they may crowd the gear.
    l.cells
        .iter()
        .rev()
        .find(|(_, r)| r.has(x, y))
        .map(|(i, _)| *i)
}

pub fn covers(l: &Layout, x: i32, y: i32) -> bool {
    l.bar.map_or(false, |b| b.has(x, y)) || l.cells.iter().any(|(_, r)| r.has(x, y))
}

/// The first gui row the strip (or the keyboard under it) takes, so the
/// console can keep its lines above it.
pub fn top(f: &Frame) -> u32 {
    layout(f).cells[0].1.y.max(0) as u32
}

fn icon(item: Item) -> [&'static str; 8] {
    match item {
        Item::Gear => [
            "...XX...", ".X.XX.X.", "..XXXX..", "XXX..XXX", "XXX..XXX", "..XXXX..", ".X.XX.X.",
            "...XX...",
        ],
        // a brush
        Item::Paint => [
            "......XX", ".....XXX", "....XXX.", "...XXX..", "..XXX...", ".X.X....", "XX......",
            "XXX.....",
        ],
        // a cube
        Item::Model => [
            "..XXXXXX", ".X....XX", "XXXXXX.X", "X....X.X", "X....X.X", "X....XX.", "XXXXXX..",
            "........",
        ],
        // angle brackets
        Item::Edit => [
            "........", "..X..X..", ".X....X.", "X......X", ".X....X.", "..X..X..", "........",
            "........",
        ],
        // a pair of notes
        Item::Sound => [
            "..XXXXXX", "..X....X", "..X....X", "..X....X", ".XX...XX", "XXX..XXX", ".X....X.",
            "........",
        ],
        // an arrow leaving a tray
        Item::Share => [
            "...XX...", "..XXXX..", ".X.XX.X.", "...XX...", "X..XX..X", "X......X", "X......X",
            "XXXXXXXX",
        ],
        // a picture
        Item::Png => [
            "XXXXXXXX", "X......X", "X.X....X", "X....X.X", "X...XXXX", "X.XXXXXX", "XXXXXXXX",
            "........",
        ],
        // a zipped folder
        Item::Zip => [
            "XXXXXXXX", "X..XX..X", "X...X..X", "X..X...X", "X...X..X", "X..XX..X", "XXXXXXXX",
            "........",
        ],
        Item::Back => [
            "........", "...X....", "..XX....", ".XXXXXXX", ".XXXXXXX", "..XX....", "...X....",
            "........",
        ],
    }
}

const BACK: Rgba<u8> = Rgba([47, 47, 47, 255]);
const EDGE: Rgba<u8> = Rgba([96, 96, 96, 255]);
const INK: Rgba<u8> = Rgba([235, 235, 235, 255]);
const LIT: Rgba<u8> = Rgba([255, 200, 60, 255]);

fn put(img: &mut RgbaImage, x: i32, y: i32, c: Rgba<u8>) {
    if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
        img.put_pixel(x as u32, y as u32, c);
    }
}

fn fill(img: &mut RgbaImage, r: Rect, c: Rgba<u8>) {
    for y in r.y..r.y + r.h {
        for x in r.x..r.x + r.w {
            put(img, x, y, c);
        }
    }
}

/// Draw the gear, and the strip when it's out, over whatever is on the layer.
pub fn paint(img: &mut RgbaImage, f: &Frame) {
    let l = layout(f);
    if let Some(bar) = l.bar {
        fill(img, bar, BACK);
        fill(img, Rect { h: 1, ..bar }, EDGE);
    }
    for (item, r) in &l.cells {
        let ox = r.x + (r.w - ICON) / 2;
        let oy = r.y + (r.h - ICON) / 2;
        if l.bar.is_none() {
            // The gear alone, over the app: a dark plate so it reads on any
            // picture.
            fill(
                img,
                Rect {
                    x: ox - 1,
                    y: oy - 1,
                    w: ICON + 2,
                    h: ICON + 2,
                },
                BACK,
            );
        }
        let lit = f.active == Some(*item) || (*item == Item::Gear && f.open);
        let ink = if lit { LIT } else { INK };
        for (row, line) in icon(*item).iter().enumerate() {
            for (col, ch) in line.bytes().enumerate() {
                if ch == b'X' {
                    put(img, ox + col as i32, oy + row as i32, ink);
                }
            }
        }
    }
}

/// The strip's state between ticks, on `Core`.
#[derive(Default)]
pub struct State {
    /// Showing share's choices.
    pub share: bool,
    /// The console was open last tick: its closing puts the keyboard away.
    was_open: bool,
    /// Window pixels a docked soft keyboard covers at the bottom. The host
    /// keeps it current (`chor` on Android); 0 anywhere else.
    pub keyboard_px: f32,
    /// What was last handed to the gui.
    shown: Option<Frame>,
}

/// Finger-sized cells on a phone (48dp, Android's own minimum); the smallest
/// on a desktop, where a mouse hits 12 pixels easily.
#[cfg(any(target_os = "android", target_os = "ios"))]
const TOUCH_DP: f32 = 48.;
#[cfg(not(any(target_os = "android", target_os = "ios")))]
const TOUCH_DP: f32 = 0.;

#[cfg(all(feature = "studio", feature = "headed"))]
impl crate::root::Core {
    /// The gear is offered wherever the console is: not to a locked app, and
    /// not to a cart whose host gave it no console.
    fn strip_enabled(&self) -> bool {
        self.policy.console && !self.global.locked
    }

    pub fn strip_frame(&self) -> Frame {
        let gui = self.gui.size;
        let win = (
            self.gfx.size.width.max(1) as f32,
            self.gfx.size.height.max(1) as f32,
        );
        let scale = self.gfx.scale as f32;
        // window pixels -> gui pixels
        let (sx, sy) = (gui[0] as f32 / win.0, gui[1] as f32 / win.1);
        let inset = self.global.safe_inset;
        let dps = (win.0.min(win.1)) / scale.max(0.1);
        let tablet = TOUCH_DP > 0. && dps >= 600.;
        let overlay = |dir| self.bundle_manager.find_by_directory(dir).is_some();
        use crate::command::{EDIT_OVERLAY, MODEL_OVERLAY, PAINT_OVERLAY, SOUND_OVERLAY};
        let active = [
            (PAINT_OVERLAY, Item::Paint),
            (MODEL_OVERLAY, Item::Model),
            (EDIT_OVERLAY, Item::Edit),
            (SOUND_OVERLAY, Item::Sound),
        ]
        .into_iter()
        .find(|(dir, _)| overlay(dir))
        .map(|(_, item)| item);
        Frame {
            gui: (gui[0], gui[1]),
            cell: ((TOUCH_DP * scale * sx.min(sy)).ceil() as u32)
                .max(MIN_CELL)
                .min(gui[1] / 4),
            keyboard: (self.strip.keyboard_px * sy).round() as u32,
            bottom: (inset[2] * sy).ceil() as u32,
            right: (inset[1] * sx).ceil() as u32,
            right_justified: win.1 > win.0 || tablet,
            open: self.global.console,
            share: self.strip.share,
            active,
        }
    }

    fn strip_console(&mut self, open: bool) {
        self.global.console = open;
        if open {
            self.gui.enable_console(&self.loggy);
        } else {
            self.gui.disable_console();
        }
    }

    /// A press (a click or a finger landing) at a 0..1 position in the
    /// window. True when the gear or strip took it, so the app never sees it.
    pub fn strip_press(&mut self, x: f32, y: f32) -> bool {
        if !self.strip_enabled() {
            return false;
        }
        let frame = self.strip_frame();
        let l = layout(&frame);
        let (px, py) = (
            (x * frame.gui.0 as f32) as i32,
            (y * frame.gui.1 as f32) as i32,
        );
        use crate::host::Request;
        let Some(item) = hit(&l, px, py) else {
            // A press on the open console brings the keyboard back if the
            // person dismissed it.
            if frame.open {
                self.requests.push(Request::Keyboard(true));
            }
            return covers(&l, px, py);
        };
        match item {
            Item::Gear => {
                self.strip.share = false;
                self.strip_console(!frame.open);
                if !frame.open {
                    self.requests.push(Request::Keyboard(true));
                }
            }
            Item::Paint | Item::Model | Item::Edit | Item::Sound => {
                if self.bundle_manager.edit_target().is_none() {
                    self.loggy.log(
                        crate::log::LogType::Config,
                        "the tools work on an app: `new name` or `load name` first",
                    );
                    return true;
                }
                // The tool takes the screen: console and strip step aside.
                self.strip_console(false);
                match item {
                    Item::Paint => crate::command::toggle_paint_overlay(self),
                    Item::Model => crate::command::toggle_model_overlay(self),
                    Item::Edit => crate::command::toggle_edit_overlay(self),
                    _ => crate::command::toggle_sound_overlay(self),
                }
            }
            Item::Share => self.strip.share = true,
            Item::Back => self.strip.share = false,
            Item::Png | Item::Zip => {
                self.strip.share = false;
                self.strip_export(item == Item::Zip);
            }
        }
        true
    }

    /// Pack the app being worked on into a `.game.png` or a `.zip` beside its
    /// folder, and hand the file to the host to share.
    fn strip_export(&mut self, zip: bool) {
        let dir = self
            .bundle_manager
            .edit_target()
            .and_then(|id| self.bundle_manager.bundles.get(&id))
            .and_then(|b| b.get_directory())
            .filter(|d| crate::builtin::scripts(d).is_none())
            .map(|d| crate::asset::resolve_app_path(d));
        let Some(dir) = dir.filter(|d| d.is_dir()) else {
            self.loggy.log(
                crate::log::LogType::ConfigError,
                "share: there is no app folder open to pack (load or make one first)",
            );
            return;
        };
        #[cfg(target_arch = "wasm32")]
        let packed: Result<std::path::PathBuf, String> =
            Err("a browser has no folder to pack into".into());
        #[cfg(not(target_arch = "wasm32"))]
        let packed = crate::asset::export_app(&dir, zip, &mut self.loggy);
        match packed {
            Ok(path) => {
                let msg = format!("packed {}", path.display());
                self.loggy.log(crate::log::LogType::Config, &msg);
                self.gui.push_notif(&msg);
                self.requests.push(crate::host::Request::Share {
                    path,
                    mime: if zip { "application/zip" } else { "image/png" },
                });
            }
            Err(e) => self.loggy.log(
                crate::log::LogType::ConfigError,
                &format!("share: couldn't pack {}: {}", dir.display(), e),
            ),
        }
    }

    /// Once a tick: follow the console (the backtick, a load and `Policy` all
    /// open and close it too) and redraw when anything changed.
    pub fn strip_tick(&mut self) {
        if !self.strip_enabled() {
            if self.strip.shown.take().is_some() {
                self.gui.set_chrome(None);
            }
            self.strip.was_open = false;
            return;
        }
        let open = self.global.console;
        if self.strip.was_open && !open {
            self.strip.share = false;
            self.requests.push(crate::host::Request::Keyboard(false));
        }
        self.strip.was_open = open;
        let frame = self.strip_frame();
        if self.strip.shown.as_ref() != Some(&frame) {
            self.strip.shown = Some(frame.clone());
            self.gui.set_chrome(Some(frame));
            let (w, h) = self.gui.get_console_size();
            self.loggy.set_dimensions(w, h);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> Frame {
        Frame {
            gui: (320, 240),
            cell: 12,
            ..Default::default()
        }
    }

    #[test]
    fn closed_is_just_the_gear_in_the_corner() {
        let l = layout(&frame());
        assert_eq!(l.bar, None);
        assert_eq!(l.cells.len(), 1);
        let (item, r) = l.cells[0];
        assert_eq!(item, Item::Gear);
        assert_eq!((r.x + r.w, r.y + r.h), (320, 240));
        assert_eq!(hit(&l, 319, 239), Some(Item::Gear));
        assert_eq!(hit(&l, 100, 100), None);
        assert!(!covers(&l, 100, 239));
    }

    #[test]
    fn the_gear_clears_the_system_bars() {
        let f = Frame {
            bottom: 20,
            right: 7,
            ..frame()
        };
        let r = layout(&f).cells[0].1;
        assert_eq!((r.x + r.w, r.y + r.h), (313, 220));
    }

    #[test]
    fn open_centres_the_tools_and_spans_the_screen() {
        let f = Frame {
            open: true,
            ..frame()
        };
        let l = layout(&f);
        let bar = l.bar.unwrap();
        assert_eq!((bar.x, bar.w, bar.y + bar.h), (0, 320, 240));
        assert_eq!(l.cells.len(), 1 + TOOLS.len());
        let first = l.cells[1].1;
        let last = l.cells[TOOLS.len()].1;
        // centred: equal space either side of the run
        assert_eq!(first.x, 320 - (last.x + last.w));
        assert_eq!(hit(&l, first.x + 1, first.y + 1), Some(Item::Paint));
        assert_eq!(hit(&l, last.x + 1, last.y + 1), Some(Item::Share));
        // the bar between icons swallows the press without acting
        assert_eq!(hit(&l, 2, 235), None);
        assert!(covers(&l, 2, 235));
    }

    #[test]
    fn right_justified_tools_sit_against_the_gear() {
        let f = Frame {
            open: true,
            right_justified: true,
            ..frame()
        };
        let l = layout(&f);
        let gear = l.cells[0].1;
        let last = l.cells[TOOLS.len()].1;
        assert_eq!(last.x + last.w, gear.x);
    }

    #[test]
    fn a_docked_keyboard_lifts_the_strip_onto_it() {
        let f = Frame {
            open: true,
            keyboard: 100,
            bottom: 20,
            ..frame()
        };
        let l = layout(&f);
        let bar = l.bar.unwrap();
        assert_eq!(bar.y + bar.h, 140);
        assert_eq!(top(&f), 128);
        for (_, r) in &l.cells {
            assert_eq!(r.y, bar.y);
        }
    }

    #[test]
    fn share_swaps_the_tools_for_its_choices() {
        let f = Frame {
            open: true,
            share: true,
            ..frame()
        };
        let l = layout(&f);
        let items: Vec<Item> = l.cells.iter().map(|c| c.0).collect();
        assert_eq!(items, [Item::Gear, Item::Png, Item::Zip, Item::Back]);
    }

    #[test]
    fn nothing_leaves_a_tiny_screen() {
        let f = Frame {
            gui: (40, 20),
            cell: 30,
            open: true,
            ..Default::default()
        };
        for (_, r) in layout(&f).cells {
            assert!(r.x >= 0 && r.y >= 0 && r.x + r.w <= 40 && r.y + r.h <= 20);
        }
    }

    #[test]
    fn paint_draws_only_where_the_layout_says() {
        let f = Frame {
            open: true,
            ..frame()
        };
        let mut img = RgbaImage::new(320, 240);
        paint(&mut img, &f);
        let bar = layout(&f).bar.unwrap();
        for (x, y, p) in img.enumerate_pixels() {
            if p.0[3] != 0 {
                assert!(bar.has(x as i32, y as i32), "stray pixel at {x},{y}");
            }
        }
        // closed: the gear and its plate, nothing else
        let mut img = RgbaImage::new(320, 240);
        paint(&mut img, &frame());
        let drawn = img.pixels().filter(|p| p.0[3] != 0).count();
        assert_eq!(drawn, 100);
    }
}

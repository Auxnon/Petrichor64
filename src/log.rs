// use lazy_static::lazy_static;
use std::sync::mpsc::{channel, Receiver, Sender};
// lazy_static! {
//     pub static ref LOG: std::sync::mpsc::channel = std::sync::mpsc::channel::<String>(100);
// }
pub struct Loggy {
    buffer: Vec<String>,
    log_dirty: bool,
    history_buffer: Vec<String>,
    history_it: usize,
    offset: f32,
    current_line: String,
    sender: Sender<(LogType, String)>,
    receiver: Receiver<(LogType, String)>,
    current_length: usize,
    buffer_dimensions: (usize, usize),
}

pub enum LogType {
    Lua,
    LuaSys,
    LuaError,
    LuaSysError,
    World,
    WorldError,
    /** Generic asset and configuration log */
    Config,
    /** Generic asset and configuration error */
    ConfigError,
    /** Texture building and formatting log*/
    Texture,
    /** Texture building and formatting error */
    TextureError,
    Model,
    ModelError,
    CoreError,
    IoError,
    Sys,
    Debug,
    /** only used for headless */
    Notification,
}

impl Loggy {
    pub fn new() -> Loggy {
        let (sender, receiver) = channel();
        Loggy {
            buffer: vec![],
            log_dirty: false,
            history_buffer: vec!["".to_string()],
            history_it: 0,
            offset: 0.,
            current_line: String::new(),
            sender,
            receiver,
            current_length: 0,
            buffer_dimensions: (0, 0),
            // queue: vec![],
        }
    }

    /** USER: adds text to current line, always user typed */
    pub fn add(&mut self, str: String) {
        // let mut s = str.lines().map(|l| l.to_string()).collect::<Vec<String>>();
        // let last=s.last();
        // s.truncate(s.len()-1);

        self.current_line.push_str(&str);

        self.log_dirty = true;
    }

    /** USER: clear terminal */
    pub fn clear(&mut self) {
        self.buffer.clear();
        self.log_dirty = true;
        self.current_length = 0;
    }

    /** USER: hit return, push down buffer for an output as it's own line, if any, and then a new line for input. return is used to activate as a command */
    pub fn carriage(&mut self) -> Option<String> {
        let s = self.current_line.clone();
        self.history_buffer.push(s.clone());
        self.buffer.push(format!(">{}", s));
        self.current_line = "".to_string();

        self.log_dirty = true;
        self.history_it = 0;
        if s.len() > 0 {
            return Some(s);
        }
        None

        // buffer.lock().push("".to_string());
    }

    pub fn get_line(&self) -> String {
        self.current_line.clone()
    }

    /** USER: populates current line with last issued command, if any*/
    pub fn history_up(&mut self) {
        let mut it = self.history_it;

        it += 1;
        if it > self.history_buffer.len() {
            it = self.history_buffer.len();
        }
        let s = self.history_buffer[(self.history_buffer.len() - it)].clone();
        println!("up com {} len {} it {}", s, self.history_buffer.len(), it);
        self.history_it = it;
        self.current_line = s.clone();
        self.log_dirty = true;
    }

    pub fn history_down(&mut self) {
        let mut it = self.history_it;

        if it > 0 {
            it -= 1;
        }

        if it > 0 {
            let s = self.history_buffer[(self.history_buffer.len() - it)].clone();
            self.current_line = s.clone();
            self.log_dirty = true;
        } else {
            self.current_line = "".to_string();
            self.log_dirty = true;
        }
        self.history_it = it;
    }

    /** USER: backspace, remove character from current line, if any */
    pub fn back(&mut self) {
        let c = self.current_line.len();
        if self.current_line.len() > 1 {
            self.current_line.remove(c - 1);
        } else {
            self.current_line.clear();
        }
        self.log_dirty = true;
    }

    /** SYS: console out, decorated as an error TBD */
    // pub fn error(&self, str: &str) {
    //     self._print(str, false);
    // }

    /** SYS: log out */
    pub fn log(&mut self, _log_type: LogType, str: &str) {
        #[cfg(feature = "headed")]
        self._print(str, false);
        println!("~{}", str);
    }

    /** SYS: Well this looks dumb, just take my word for it*/
    // pub fn print(str: String) {
    //     _print(str, true);
    // }

    /** SYS: only used by */
    pub fn _print(&mut self, str: &str, skip_first: bool) {
        // format!("{}\n{}", self.text, str);
        //buffer.lock().push_str(&str);
        //let mut n = "\n".to_string();
        //n.push_str(&str);
        // self.queue.push(str.to_string());
        let mut s = str.lines().map(|l| l.to_string()).collect::<Vec<String>>();
        // println!("lines {}", s.len());
        if skip_first {
            match self.buffer.last_mut() {
                Some(o) => {
                    o.push_str(&s[0]);
                }
                None => self.buffer.push(s[0].clone()),
            }
            self.buffer.append(&mut s[1..s.len()].to_vec());
        } else {
            self.buffer.append(&mut s);
        }
        let n = self.buffer.len();
        if n > 150 {
            self.buffer.drain(0..50);
        }
        self.current_length = self.buffer.len();
        self.log_dirty = true;
    }

    pub fn set_dimensions(&mut self, width: u32, height: u32) {
        self.buffer_dimensions = (width as usize, height as usize);
        self.log_dirty = true;
    }

    pub fn scroll(&mut self, delta: f32) {
        self.offset += delta;

        let cap = if self.buffer_dimensions.1 >= self.current_length {
            (0, self.current_length)
        } else {
            (0, self.current_length - self.buffer_dimensions.1)
        };
        // println!("cap {}", cap);
        self.offset = self.offset.clamp(cap.0 as f32, cap.1 as f32);
        // println!(
        //     "scroll {} with cap {} length {} dim {} actual len {}",
        //     self.offset,
        //     cap.1,
        //     self.current_length,
        //     self.buffer_dimensions.1,
        //     self.buffer.len()
        // );
        self.log_dirty = true;
    }

    pub fn check_width(buf: Vec<String>, width: usize) -> Vec<String> {
        let mut out = vec![];
        for l in buf {
            let mut h = l;
            while h.len() > width {
                let (a, b) = h.split_at(width);
                out.push(a.to_string());
                h = b.to_string();
            }
            out.push(h);
        }
        out
    }

    pub fn get(&self) -> String {
        let (width, height) = self.buffer_dimensions;
        let l = self.current_length;

        let pre_buf = if l < height {
            self.buffer.clone()
        } else {
            let offset = (self.offset).floor() as usize;

            let contro_height = l - (height - 1);
            let (deg, cap) = if contro_height < offset {
                (0, l)
            } else {
                (contro_height - offset, l - offset)
            };

            self.buffer[(deg)..cap].to_vec()
        };

        let next_buf = Self::check_width(pre_buf, width);
        let prel = next_buf.len();

        let mut buf = if prel < height {
            next_buf.join("\n")
        } else {
            next_buf[(prel - (height - 1))..].join("\n")
        };
        buf.push_str("\n>");
        let cur = self.current_line.clone();
        if cur.len() > width {
            let l = cur.len();
            buf.push_str(&cur[l - width..l]);
        } else {
            buf.push_str(&cur);
        }
        buf
    }
    pub fn listen(&mut self) {
        // println!("listen");
        if let Ok((t, s)) = self.receiver.try_recv() {
            self.log(t, &s);
        }
    }
    /** Check for any pending messages and return true if the log has new content or not*/
    pub fn is_dirty_and_listen(&mut self) -> bool {
        self.listen();

        self.log_dirty
    }
    /** set dirty */
    pub fn clean(&mut self) {
        self.log_dirty = false;
    }

    pub fn make_sender(&self) -> Sender<(LogType, String)> {
        self.sender.clone()
    }
}

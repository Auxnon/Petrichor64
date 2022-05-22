use lazy_static::lazy_static;
use parking_lot::Mutex;
lazy_static! {
    static ref buffer: Mutex<Vec<String>> = Mutex::new(vec![]);
    static ref log_dirty: Mutex<bool> = Mutex::new(false);
    static ref history_buffer: Mutex<String> = Mutex::new(String::new());
    static ref current_line: Mutex<String> = Mutex::new(String::new());
}

/** USER: adds text to current line, always user typed */
pub fn add(str: String) {
    // let mut s = str.lines().map(|l| l.to_string()).collect::<Vec<String>>();
    // let last=s.last();
    // s.truncate(s.len()-1);

    match current_line.try_lock() {
        Some(mut g) => g.push_str(&str),
        _ => (),
    }
    // current_line.lock().push_str(&str);

    *log_dirty.lock() = true;
}

/** USER: adds a new line */
pub fn next_line() {
    buffer.lock().push("".to_string());
    *log_dirty.lock() = true;
}

/** USER: hit return, push down buffer for an output as it's own line, if any, and then a new line for input. return is used to activate as a command */
pub fn carriage() -> Option<String> {
    let s = current_line.lock().clone();
    *history_buffer.lock() = s.clone();
    buffer.lock().push(format!(">{}", s));
    *current_line.lock() = "".to_string();

    *log_dirty.lock() = true;

    match buffer.lock().last() {
        Some(s) => {
            *log_dirty.lock() = true;
            *history_buffer.lock() = s.to_string();
            Some(s.to_string())
        }
        _ => None,
    }
    // buffer.lock().push("".to_string());
}

/** USER: popualtes current line with last issued command, if any*/
pub fn history() {
    match buffer.lock().last_mut() {
        Some(o) => {
            let s = history_buffer.lock();
            // s.clone()
            *o = s.clone();
            *log_dirty.lock() = true;
        }
        None => {}
    }
}

/** USER: backspace, remove character from current line, if any */
pub fn back() {
    let mut s = current_line.lock();
    let c = s.len();
    if s.len() > 1 {
        s.remove(c - 1);
    } else {
        *s = String::new();
    }
    *log_dirty.lock() = true;
}

/** SYS: console out, decorated as an error TBD */
pub fn error(str: String) {
    _print(str, false);
}

/** SYS: log out */
pub fn log(str: String) {
    _print(str, false);
}

/** SYS: Well this looks dumb, just take my word for it*/
pub fn print(str: String) {
    _print(str, true);
}

/** SYS: only used by */
pub fn _print(str: String, skip_first: bool) {
    // format!("{}\n{}", self.text, str);
    //buffer.lock().push_str(&str);
    //let mut n = "\n".to_string();
    //n.push_str(&str);
    let mut s = str.lines().map(|l| l.to_string()).collect::<Vec<String>>();
    // println!("lines {}", s.len());
    if skip_first {
        match buffer.lock().last_mut() {
            Some(o) => {
                o.push_str(&s[0]);
            }
            None => buffer.lock().push(s[0].clone()),
        }
        buffer.lock().append(&mut s[1..s.len()].to_vec());
    } else {
        buffer.lock().append(&mut s);
    }
    let n = buffer.lock().len();
    if n > 150 {
        buffer.lock().drain(0..50);
    }
    *log_dirty.lock() = true;
}

pub fn get(height: usize) -> String {
    println!("get len {}, height{}", buffer.lock().len(), height);
    let l = buffer.lock().len();

    let mut buf = if l < height {
        buffer.lock().join("\n")
    } else {
        buffer.lock()[(l - (height - 1))..].join("\n")
    };
    buf.push_str("\n>");
    buf.push_str(&current_line.lock().clone());
    buf
}
pub fn is_dirty() -> bool {
    *log_dirty.lock()
}
pub fn clean() {
    *log_dirty.lock() = false;
}

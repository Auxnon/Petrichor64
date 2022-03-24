use lazy_static::lazy_static;
use parking_lot::Mutex;
lazy_static! {
    static ref buffer: Mutex<Vec<String>> = Mutex::new(vec![]);
    static ref log_dirty: Mutex<bool> = Mutex::new(false);
    static ref history_buffer: Mutex<String> = Mutex::new(String::new());
}
pub fn add(str: String) {
    // let mut s = str.lines().map(|l| l.to_string()).collect::<Vec<String>>();
    // let last=s.last();
    // s.truncate(s.len()-1);
    match buffer.lock().last_mut() {
        Some(o) => {
            o.push_str(&str);
        }
        None => buffer.lock().push(str),
    }
    *log_dirty.lock() = true;
}
pub fn next_line() {
    buffer.lock().push("".to_string());
    *log_dirty.lock() = true;
}
pub fn carriage() -> Option<String> {
    let last = match buffer.lock().last() {
        Some(s) => Some(s.to_owned()),
        _ => None,
    };
    buffer.lock().push("".to_string());
    *log_dirty.lock() = true;
    match last.clone() {
        Some(s) => *history_buffer.lock() = s,
        None => {}
    }
    last
}

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
pub fn back() {
    match buffer.lock().last_mut() {
        Some(s) => {
            if s.len() > 1 {
                s.remove(s.len() - 1);
            } else {
                *s = String::new();
            }
            *log_dirty.lock() = true;
        }
        _ => (),
    };
}

pub fn error(str: String) {
    _print(str, false);
}

pub fn log(str: String) {
    _print(str, false);
}

pub fn print(str: String) {
    _print(str, true);
}

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
    let n = buffer.lock().len() - height;
    // println!("get len {}, n{} height{}", buffer.lock().len(), n, height);
    let s = buffer.lock()[n..].join("\n");
    // println!(" lines {} ", s.split("\n").collect::<Vec<&str>>().len());
    s
}
pub fn is_dirty() -> bool {
    *log_dirty.lock()
}
pub fn clean() {
    *log_dirty.lock() = false;
}

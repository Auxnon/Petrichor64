use lazy_static::lazy_static;
use parking_lot::Mutex;
lazy_static! {
    static ref BUFFER: Mutex<Vec<String>> = Mutex::new(vec![]);
    static ref LOG_DIRTY: Mutex<bool> = Mutex::new(false);
    static ref HISTORY_BUFFER: Mutex<Vec<String>> = Mutex::new(vec!["".to_string()]);
    static ref HISTORY_IT: Mutex<usize> = Mutex::new(0);
    static ref CURRENT_LINE: Mutex<String> = Mutex::new(String::new());
}

/** USER: adds text to current line, always user typed */
pub fn add(str: String) {
    // let mut s = str.lines().map(|l| l.to_string()).collect::<Vec<String>>();
    // let last=s.last();
    // s.truncate(s.len()-1);

    match CURRENT_LINE.try_lock() {
        Some(mut g) => g.push_str(&str),
        _ => (),
    }
    // current_line.lock().push_str(&str);

    *LOG_DIRTY.lock() = true;
}

/** USER: adds a new line */
// pub fn next_line() {
//     BUFFER.lock().push("".to_string());
//     *LOG_DIRTY.lock() = true;
// }

/** USER: clear terminal */
pub fn clear() {
    BUFFER.lock().clear();
    *LOG_DIRTY.lock() = true;
}

/** USER: hit return, push down buffer for an output as it's own line, if any, and then a new line for input. return is used to activate as a command */
pub fn carriage() -> Option<String> {
    let s = CURRENT_LINE.lock().clone();
    HISTORY_BUFFER.lock().push(s.clone());
    BUFFER.lock().push(format!(">{}", s));
    *CURRENT_LINE.lock() = "".to_string();

    *LOG_DIRTY.lock() = true;
    *HISTORY_IT.lock() = 0;
    if s.len() > 0 {
        return Some(s);
    }
    None

    // buffer.lock().push("".to_string());
}

pub fn get_line() -> String {
    CURRENT_LINE.lock().clone()
}

/** USER: populates current line with last issued command, if any*/
pub fn history_up() {
    let hist = HISTORY_BUFFER.lock();
    let mut it = *HISTORY_IT.lock();

    it += 1;
    if it > hist.len() {
        it = hist.len();
    }
    let s = hist[(hist.len() - it)].clone();
    println!("up com {} len {} it {}", s, hist.len(), it);
    *HISTORY_IT.lock() = it;
    *CURRENT_LINE.lock() = s.clone();
    *LOG_DIRTY.lock() = true;
}

pub fn history_down() {
    let hist = HISTORY_BUFFER.lock();
    let mut it = *HISTORY_IT.lock();

    if it > 0 {
        it -= 1;
    }

    if it > 0 {
        let s = hist[(hist.len() - it)].clone();
        *CURRENT_LINE.lock() = s.clone();
        *LOG_DIRTY.lock() = true;
    } else {
        *CURRENT_LINE.lock() = "".to_string();
        *LOG_DIRTY.lock() = true;
    }
    *HISTORY_IT.lock() = it;
}

/** USER: backspace, remove character from current line, if any */
pub fn back() {
    let mut s = CURRENT_LINE.lock();
    let c = s.len();
    if s.len() > 1 {
        s.remove(c - 1);
    } else {
        *s = String::new();
    }
    *LOG_DIRTY.lock() = true;
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
// pub fn print(str: String) {
//     _print(str, true);
// }

/** SYS: only used by */
pub fn _print(str: String, skip_first: bool) {
    // format!("{}\n{}", self.text, str);
    //buffer.lock().push_str(&str);
    //let mut n = "\n".to_string();
    //n.push_str(&str);
    let mut s = str.lines().map(|l| l.to_string()).collect::<Vec<String>>();
    // println!("lines {}", s.len());
    if skip_first {
        match BUFFER.lock().last_mut() {
            Some(o) => {
                o.push_str(&s[0]);
            }
            None => BUFFER.lock().push(s[0].clone()),
        }
        BUFFER.lock().append(&mut s[1..s.len()].to_vec());
    } else {
        BUFFER.lock().append(&mut s);
    }
    let n = BUFFER.lock().len();
    if n > 150 {
        BUFFER.lock().drain(0..50);
    }
    *LOG_DIRTY.lock() = true;
}

pub fn get(width: usize, height: usize) -> String {
    // println!("get len {}, height{}", BUFFER.lock().len(), height);
    let l = BUFFER.lock().len();

    let mut buf = if l < height {
        BUFFER.lock().join("\n")
    } else {
        BUFFER.lock()[(l - (height - 1))..].join("\n")
    };
    buf.push_str("\n>");
    let cur = CURRENT_LINE.lock().clone();
    if cur.len() > width {
        let l = cur.len();
        buf.push_str(&cur[l - width..l]);
    } else {
        buf.push_str(&cur);
    }
    buf
}
pub fn is_dirty() -> bool {
    *LOG_DIRTY.lock()
}
pub fn clean() {
    *LOG_DIRTY.lock() = false;
}

#[macro_export]
macro_rules! lg{
    ($($arg:tt)*) => {{
           {
            let st=format!("::{}",format!($($arg)*));
            println!("{}",st);
            crate::log::log(st);
           }
       }
   }
}

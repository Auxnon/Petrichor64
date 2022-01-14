use lazy_static::lazy_static;
use parking_lot::Mutex;
lazy_static! {
    static ref buffer: Mutex<String> = Mutex::new(String::new());
    static ref log_dirty: Mutex<bool> = Mutex::new(false);
}

pub fn log(str: String) {
    // format!("{}\n{}", self.text, str);
    //buffer.lock().push_str(&str);

    let mut n = "\n".to_string();
    n.push_str(&str);
    buffer.lock().push_str(&n);
    *log_dirty.lock() = true;
    println!("{}", str);
}
pub fn get() -> String {
    buffer.lock().clone()
}
pub fn is_dirty() -> bool {
    *log_dirty.lock()
}
pub fn clean() {
    *log_dirty.lock() = false;
}

use lazy_static::lazy_static;
use parking_lot::Mutex;
lazy_static! {
    static ref buffer: Mutex<Vec<String>> = Mutex::new(vec![]);
    static ref log_dirty: Mutex<bool> = Mutex::new(false);
}

pub fn log(str: String) {
    // format!("{}\n{}", self.text, str);
    //buffer.lock().push_str(&str);

    //let mut n = "\n".to_string();
    //n.push_str(&str);
    println!("{}", str);

    buffer.lock().push(str);
    let n = buffer.lock().len();
    if n > 150 {
        buffer.lock().drain(0..50);
    }
    *log_dirty.lock() = true;
}
pub fn get(height: usize) -> String {
    let n = buffer.lock().len() - height;
    buffer.lock()[n..].join("\n")
}
pub fn is_dirty() -> bool {
    *log_dirty.lock()
}
pub fn clean() {
    *log_dirty.lock() = false;
}

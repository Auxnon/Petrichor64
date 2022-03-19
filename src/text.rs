use lazy_static::lazy_static;
use parking_lot::Mutex;

lazy_static! {
    static ref contents: Mutex<Vec<String>> = Mutex::new(vec![]);
}

pub fn add_text(str: &String, neg: u16) {
    let n = contents.lock().len();
    // let t = contents.lock().get_mut(n - 1);

    match contents.lock().last() {
        Some(t) => {
            println!("last string: {} ", t);
            if t.len() > 51 {
                contents.lock().push(str.to_owned());
            } else {
                t.to_owned().push_str(str);
                contents.lock().last().replace(t);
                //*t += str;
            }
        }
        _ => {
            contents.lock().push(str.to_owned());
        }
    }
}
// pub fn get_range(){
//     let l=contents.lock().iter().enumerate()
// }

use lazy_static::lazy_static;
use parking_lot::Mutex;

lazy_static! {
    static ref contents: Mutex<Vec<String>> = Mutex::new(vec![]);
}

pub fn add_text(str: &String, neg: u16) {
    // let n = contents.lock().len();
    // let t = contents.lock().get_mut(n - 1);

    // match contents.lock().last() {
    //     Some(t) => {
    //         if t.len() > 51 {
    //         } else {
    //             contents.lock().last() = t + str;
    //             //*t += str;
    //         }
    //     }
    //     _ => {}
    // }
}

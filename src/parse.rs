use std::{fmt::Write, fs::File, io::BufReader, io::Read, path::PathBuf};

pub fn test(s: &String) {
    let path = PathBuf::new().join(s);
    match File::open(&path) {
        Ok(f) => {
            let mut reader = BufReader::new(f);
            let mut buf = String::new();
            match reader.read_to_string(&mut buf) {
                Ok(o) => parser(&buf),
                _ => {}
            };
        }
        _ => {}
    }
}
#[derive(PartialEq, Eq)]
enum Oper {
    Plus,
    Minus,
    Func,
    Func2,
    Arrow,
    Nil,
}

fn parser(s: &String) {
    let before = s.clone();
    let mut cur_key = "".to_string();
    let mut new_word = false;
    let mut op: Oper = Oper::Nil;
    let mut out = "".to_string();
    // let mut last=' ';
    // let mut testing = "".to_string();

    for c in s.chars() {
        let mut changed = false;
        match c {
            '+' => op = Oper::Plus,
            '=' => {
                match op {
                    Oper::Plus => {
                        changed = true;
                        out.pop();
                        write!(&mut out, "={}+", cur_key);
                    }
                    Oper::Arrow => {
                        op = Oper::Nil;
                    }
                    _ => op = Oper::Arrow,
                };
            }
            '>' => {
                if op == Oper::Arrow {
                    changed = true;
                    let h = cur_key.len() - 1;
                    let olen = out.len();
                    out.drain((olen - h)..olen);
                    write!(&mut out, "function ({}) ", cur_key);
                }
            }
            ' ' => {
                new_word = true;
                if op == Oper::Func2 {
                    op = Oper::Nil;
                    out.pop();
                    out.pop();
                    write!(&mut out, "function",);
                }
            }
            '\n' => {
                new_word = true;
                if op == Oper::Func2 {
                    op = Oper::Nil;
                    out.pop();
                    out.pop();
                    write!(&mut out, "function",);
                }
            }
            'f' => op = Oper::Func,
            'n' => {
                if op == Oper::Func {
                    op = Oper::Func2
                }
            }
            _ => {
                if new_word {
                    cur_key.clear();
                    new_word = false;
                }
                op = Oper::Nil;
                cur_key.push(c);
            }
        }
        if !changed {
            out.push(c);
        }
    }
    println!("BEFORE {}", before);
    println!("AFTER {}", out);
}

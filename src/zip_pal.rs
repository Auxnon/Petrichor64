use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::PathBuf;
use std::{fs::File, path::Path};

use zip::write::FileOptions;

use crate::lg;

/**
 * Squish a data file on to the end of an image file. PoC.
 */
// fn smash(thumbnail: &String, data: &String, out: &String) {
//     let mut file1 = get_file_buffer(thumbnail);
//     let mut file2 = get_file_buffer(data);
//     let new_file = File::create(&Path::new(out)).unwrap();

//     file1.append(&mut file2);
//     let mut writer = BufWriter::new(new_file);
//     write_check(writer.write(file1.as_slice()));
// }

/** Expand a packed image back into a seperate file, based on the end position of the image file. PoC */
// fn stretch(source: &String, out_name: &String) {
//     let mut file1 = get_file_buffer(source);

//     let mut v = vec![];
//     let mut toggle = false;
//     file1 = file1[1..file1.len()].to_vec();
//     let mut iter = 0;
//     let test = [73, 69, 78, 68, 174, 66, 96, 130];

//     for chunk in file1.chunks(1) {
//         if !toggle {
//             // print!("{:?}_", chunk);
//             if chunk[0] == test[iter] {
//                 if iter < 7 {
//                     iter += 1;
//                 } else {
//                     toggle = true;
//                 }
//             } else {
//                 iter = 0;
//             }
//         } else {
//             v.append(&mut chunk.to_vec());
//         }
//     }
//     println!("stretch size {}", v.len());

//     let new_file = File::create(&Path::new(out_name)).unwrap();
//     let mut writer = BufWriter::new(new_file);
//     write_check(writer.write(v.as_slice()));
// }

/** load a file and return as a u8 vector buffer */
pub fn get_file_buffer(path_str: &String) -> Vec<u8> {
    let path = PathBuf::new().join(path_str);
    // println!("get filepath {:?}", path);
    get_file_buffer_from_path(path)
}

pub fn get_file_buffer_from_path(path: PathBuf) -> Vec<u8> {
    let mut v = vec![];
    match File::open(&path) {
        Ok(f) => {
            let mut reader = BufReader::new(f);
            // reader.re

            match reader.read_to_end(&mut v) {
                Ok(_) => {
                    //log(format!("buffer size {} for {}", x, &path_str)),
                }
                Err(_) => {}
            };
        }
        _ => {}
    }
    v
}

/** read provided source string paths into a zip file, and smash it on to the end of an image file (see squish for simple smash) */
pub fn pack_zip(sources: Vec<String>, thumb: PathBuf, out: &String) {
    // let zipfile = std::fs::File::open(name).unwrap();
    let mut image = get_file_buffer_from_path(thumb);
    if image.len() > 0 {
        crate::lg!("using icon of {} bytes", image.len());

        // let new_file = File::create(&Path::new("temp")).unwrap();
        let v = Vec::new();
        let c = Cursor::new(v);

        let mut zip = zip::ZipWriter::new(c);
        let options = FileOptions::default();

        for source in sources {
            //.to_string();
            // zip.add_directory(s, options);
            //zip::ZipWriter::start_file;
            match zip.start_file(
                &source,
                options.compression_method(zip::CompressionMethod::Stored),
            ) {
                Ok(_) => {}
                Err(err) => {
                    log(format!("zipping error: {}", err));
                }
            }

            let buff = get_file_buffer(&source);
            let buffy = buff.as_slice();
            // println!("buffy size {} and source {}", buffy.len(), source);
            let re = zip.write(buffy);
            if re.is_err() {
                log(format!("zipping error: {}", re.unwrap()));
            } else {
                // log(format!(println!(" zip buffy size? {}", re.unwrap()));
            }
        }

        // zip.write_all(buf)
        match zip.finish() {
            Ok(mut f) => {
                f.set_position(0);

                // Read the "file's" contents into a vector
                let mut buf = Vec::new();
                f.read_to_end(&mut buf).unwrap();
                println!("zip buffer size {}", buf.len());

                image.append(&mut buf);
                let new_file = File::create(&Path::new(out)).unwrap();
                let mut writer = BufWriter::new(new_file);
                match writer.write(image.as_slice()) {
                    Ok(_) => lg("cartridge zipped!"),
                    Err(err) => log(format!("failed zipping to cartridge: {}", err)),
                }
            }
            Err(_) => todo!(),
        }
    } else {
        crate::lg!("unable to pack file as icon chosen is not available, is it in the game directory root?");
    }
}

fn write_check(res: std::io::Result<usize>) {
    match res {
        Ok(_) => {}
        Err(err) => log(format!("failed to write: {}", err)),
    }
}

/** unpacked a packed game image-zip and save the zip contents as a useable file*/
pub fn unpack_and_save(file: Vec<u8>, out: &String) {
    let v = unpack(file);
    if v.len() > 0 {
        let new_file = File::create(&Path::new(out)).unwrap();
        let mut writer = BufWriter::new(new_file);
        match writer.write(v.as_slice()) {
            Ok(_) => {
                lg!("unpacked game {} into {}.zip", out, out);
            }
            Err(err) => log(format!("cannot unpack game: {}", err)),
        }
    }
}

// MARK new pack
pub fn pack_game_bin(out: &String) -> &str {
    let mut game_buffer = get_file_buffer(&"Petrichor".to_string());
    if game_buffer.len() <= 0 {
        return "Can't find engine file";
    }

    let mut icon = get_file_buffer(&"icon.png".to_string());
    let new_file = File::create(&Path::new(out)).unwrap();
    game_buffer.append(&mut icon);
    let mut writer = BufWriter::new(new_file);
    match writer.write(game_buffer.as_slice()) {
        Ok(_) => "game packed!",
        Err(_) => "failed to pack game",
    }
}

/** unpack a packed game image-zip and load all assets into memory and return as asset-path keyed hashmap of u8 buffers  */
pub fn unpack_and_walk(
    file: Vec<u8>,
    sort: Vec<String>,
) -> HashMap<String, Vec<(String, Vec<u8>)>> {
    let v = unpack(file);
    let mut map: HashMap<String, Vec<(String, Vec<u8>)>> = HashMap::new();
    if v.len() <= 0 {
        return map;
    }
    let reader = std::io::Cursor::new(v);

    let mut archive = zip::ZipArchive::new(reader).unwrap();
    let it: Vec<String> = archive.file_names().map(|x| x.to_string()).collect();
    // let main_dir = vec![];

    for d in sort {
        println!("make {}", d);
        map.insert(d, vec![]);
    }

    for file_name in it {
        let shorter = if file_name.starts_with("./") {
            file_name[2..file_name.len()].to_string()
        } else {
            file_name.clone()
        };

        let mut part = shorter.split("/").collect::<Vec<&str>>();
        if part.len() > 1 {
            // let dir_o = part.next();
            // let name_o = part.next();

            // if dir_o.is_none() && name_o.is_none() {
            // match archive.by_name(file_name.as_str()) {
            //     Ok(file) => {main_dir.push(file)},
            //     Err(..) => {
            //         println!("?");
            //     }
            // };
            // } else {
            let dir = part[part.len() - 2];
            let name = part[part.len() - 1];
            println!(
                "check file {} and convert to dir {} and name {}",
                shorter, dir, name
            );
            crate::lg!("full {}, file {}, dir {}", file_name, name, dir);
            match archive.by_name(file_name.as_str()) {
                Ok(mut file) => match map.get_mut(&dir.to_string()) {
                    Some(ar) => {
                        let mut contents = Vec::new();
                        // println!("found file");
                        match file.read_to_end(&mut contents) {
                            Ok(_) => {}
                            _ => {}
                        }
                        ar.push((file.name().to_string(), contents));
                    }
                    _ => {}
                },
                Err(..) => {
                    println!("?");
                }
            };
            // }

            crate::lg!("list: {}", file_name);
        } else {
            crate::lg!("bad path for {}", file_name);
        }
    }

    map
}

/** Unpacked a packed game image-zip into just the zip as a u8 buffer, buffer will still need unzipping */
pub fn unpack(gamefile: Vec<u8>) -> Vec<u8> {
    // let mut gamefile = get_file_buffer(target);
    if gamefile.len() <= 0 {
        lg("file to unpack is 0 bytes!");
        return vec![];
    }
    // println!("zip file found {}", gamefile.len());

    let mut v = vec![];
    let mut toggle = false;
    let newgamefile = gamefile[1..gamefile.len()].to_vec();
    let mut iter = 0;
    let test = [73, 69, 78, 68, 174, 66, 96, 130];

    for chunk in newgamefile.chunks(1) {
        if !toggle {
            if chunk[0] == test[iter] {
                if iter < 7 {
                    iter += 1;
                } else {
                    println!("got split");
                    toggle = true;
                }
            } else {
                iter = 0;
            }
        } else {
            v.append(&mut chunk.to_vec());
        }
        //     vec_chunks.push(chunk.to_vec());
    }
    log(format!("stretch size {}", v.len()));

    v
}

/** alternative unpack method? WIP */
// pub fn walk_zip(str: &String) {
//     let zipfile = std::fs::File::open(str).unwrap();

//     let archive = zip::ZipArchive::new(zipfile).unwrap();

//     let it = archive.file_names();

//     for file_name in it {

//         // println!("list: {}", n);
//     }

//     // let mut file = match archive.by_name("gamecart.png") {
//     //     Ok(file) => file,
//     //     Err(..) => {
//     //         println!("File test/lorem_ipsum.txt not found");
//     //         return 2;
//     //     }
//     // };

//     // let mut contents = Vec::new();
//     // match file.read_to_end(&mut contents) {
//     //     Ok(size) => {
//     //         for b in contents {
//     //             print!("{}_", b);
//     //         }
//     //     }
//     //     _ => {}
//     // }
// }
/** log str */
fn lg(s: &str) {
    crate::log::log(format!("zip::{}", s));
}

/** log String */
fn log(str: String) {
    crate::log::log(format!("zip::{}", str));
}

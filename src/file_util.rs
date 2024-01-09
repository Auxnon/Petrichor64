use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Component, PathBuf};
use std::{fs::File, path::Path};

use zip::result::ZipError;
use zip::write::FileOptions;

use crate::error::P64Error;
use crate::log::{LogType, Loggy};

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
pub fn get_file_buffer(path_str: &str) -> Result<Vec<u8>, P64Error> {
    let path = PathBuf::new().join(path_str);
    // println!("get filepath {:?}", path);
    get_file_buffer_from_path(path)
}

/** write a string to a file */
pub fn write_file_string(path: PathBuf, contents: &str) -> Result<(), P64Error> {
    let mut file = match File::create(&path) {
        Ok(f) => f,
        Err(e) => return Err(P64Error::IoError(e)),
    };
    let mut writer = BufWriter::new(file);
    match writer.write(contents.as_bytes()) {
        Ok(_) => Ok(()),
        Err(e) => Err(P64Error::IoError(e)),
    }
}

/** Load file contents as buffer */
pub fn get_file_buffer_from_path(path: PathBuf) -> Result<Vec<u8>, P64Error> {
    let mut v = vec![];
    match File::open(&path) {
        Ok(f) => {
            let mut reader = BufReader::new(f);

            match reader.read_to_end(&mut v) {
                Ok(_) => {
                    //log(format!("buffer size {} for {}", x, &path_str)),
                }
                Err(e) => {
                    return Err(P64Error::IoError(e));
                }
            };
        }
        _ => {}
    }
    Ok(v)
}

/** Load file contents as utf8 string from path */
pub fn get_file_string_from_path(path: PathBuf) -> Result<String, P64Error> {
    let v = get_file_buffer_from_path(path)?;
    match String::from_utf8(v) {
        Ok(s) => Ok(s),
        Err(e) => Err(P64Error::IoUtf8Error),
    }
}

/** Scrub path to not go higher than dir */
fn scrub_path(dir: &str, path: &str) -> Result<PathBuf, P64Error> {
    let p = PathBuf::new().join(path);
    if p.components()
        .into_iter()
        .any(|x| x == Component::ParentDir)
    {
        return Err(P64Error::PermPathTraversal);
    }
    Ok(PathBuf::new().join(dir).join(path))
}

/** Load file contents as utf8 string, file path cannot go higher than dir */
pub fn get_file_string_scrubbed(dir: &str, path: &str) -> Result<String, P64Error> {
    let p = scrub_path(dir, path)?;
    get_file_string_from_path(p)
}

/** Write file with utf-8 string as contents, file path cannot go higher than dir */
pub fn write_file_string_scrubbed(dir: &str, path: &str, contents: &str) -> Result<(), P64Error> {
    let p = scrub_path(dir, path)?;
    write_file_string(p, contents)
}

fn handle_zip_error(err: ZipError, f: Option<&str>) -> P64Error {
    match err {
        zip::result::ZipError::Io(i) => P64Error::IoError(i),
        zip::result::ZipError::InvalidArchive(a) | zip::result::ZipError::UnsupportedArchive(a) => {
            P64Error::IoInvalidArchive(a)
        }
        zip::result::ZipError::FileNotFound => match f {
            Some(ff) => P64Error::IoFileNotFound(ff.into()),
            None => P64Error::IoFileNotFound("unknown".into()),
        },
    }
}
/** read provided source string paths into a zip file, and smash it on to the end of an image file (see squish for simple smash) */
pub fn pack_zip(
    sources: Vec<&str>,
    thumb: PathBuf,
    out: &str,
    loggy: &mut Loggy,
) -> Result<(), P64Error> {
    // let zipfile = std::fs::File::open(name).unwrap();
    let mut bin = get_file_buffer_from_path(thumb)?;
    if bin.len() > 0 {
        loggy.log(
            LogType::Config,
            &format!("using icon of {} bytes", bin.len()),
        );

        // let new_file = File::create(&Path::new("temp")).unwrap();
        let v = Vec::new();
        let c = Cursor::new(v);

        let mut zip = zip::ZipWriter::new(c);
        let options = FileOptions::default();

        for source in sources {
            //.to_string();
            // zip.add_directory(s, options);
            //zip::ZipWriter::start_file;
            if let Err(err) = zip.start_file(
                source,
                options.compression_method(zip::CompressionMethod::Stored),
            ) {
                return Err(handle_zip_error(err, Some(&source)));
            }

            let buff = get_file_buffer(&source)?;
            let buffy = buff.as_slice();
            if let Err(err) = zip.write(buffy) {
                loggy.log(LogType::ConfigError, &format!("zipping error: {}", err));
            }
        }

        // zip.write_all(buf)
        match zip.finish() {
            Ok(mut f) => {
                f.set_position(0);

                // Read the "file's" contents into a vector
                let mut buf = Vec::new();
                f.read_to_end(&mut buf).unwrap();
                loggy.log(LogType::Config, &format!("zip buffer size {}", buf.len()));

                bin.append(&mut buf);
                let new_file = File::create(&Path::new(out)).unwrap();
                let mut writer = BufWriter::new(new_file);
                match writer.write(bin.as_slice()) {
                    Ok(_) => {
                        loggy.log(LogType::Config, &"cartridge zipped!");
                        Ok(())
                    }
                    Err(err) => Err(P64Error::IoError(err)),
                }
            }
            Err(e) => Err(handle_zip_error(e, Some(out))),
        }
    } else {
        loggy.log(LogType::ConfigError,&"unable to pack file as icon chosen is not available, is it in the game directory root?");
        Err(P64Error::IoEmptyFile)
    }
}

// fn write_check(res: std::io::Result<usize>) {
//     match res {
//         Ok(_) => {}
//         Err(err) => log(format!("failed to write: {}", err)),
//     }
// }

/** unpacked a packed game image-zip and save the zip contents as a useable file*/
pub fn unpack_and_save(file: Vec<u8>, out: &String, loggy: &mut Loggy) {
    let v = unpack(file, loggy);
    if v.len() > 0 {
        let new_file = File::create(&Path::new(out)).unwrap();
        let mut writer = BufWriter::new(new_file);
        match writer.write(v.as_slice()) {
            Ok(_) => {
                loggy.log(
                    LogType::Config,
                    &format!("unpacked game {} into {}.zip", out, out),
                );
            }
            Err(err) => loggy.log(
                LogType::ConfigError,
                &format!("cannot unpack game: {}", err),
            ),
        }
    }
}

pub fn pack_game_bin(out: &str) -> Result<&str, P64Error> {
    let mut game_buffer = get_file_buffer(&"Petrichor".to_string())?;
    if game_buffer.len() <= 0 {
        return Err(P64Error::IoEmptyFile);
    }

    let mut icon = get_file_buffer(&"icon.png".to_string())?;
    let new_file = match File::create(&Path::new(out)) {
        Ok(f) => f,
        Err(e) => return Err(P64Error::IoError(e)),
    };
    game_buffer.append(&mut icon);
    let mut writer = BufWriter::new(new_file);
    match writer.write(game_buffer.as_slice()) {
        Ok(_) => Ok("game packed!"),
        Err(e) => Err(P64Error::IoError(e)),
    }
}

pub fn get_archive(file: Vec<u8>, loggy: &mut Loggy) -> Option<zip::ZipArchive<Cursor<Vec<u8>>>> {
    let v = unpack(file, loggy);
    if v.len() <= 0 {
        return None;
    }

    let reader = std::io::Cursor::new(v);
    match zip::ZipArchive::new(reader) {
        Ok(a) => Some(a),
        Err(e) => {
            loggy.log(
                LogType::ConfigError,
                &format!("unable to open archive: {}", e),
            );
            None
        }
    }
}

/** unpack a packed game image-zip and load all assets into memory and return as asset-path keyed hashmap of u8 buffers  */
pub fn unpack_and_walk<'a>(
    archive: &mut zip::ZipArchive<Cursor<Vec<u8>>>,
    sort: Vec<&'a str>,
    loggy: &mut Loggy,
) -> HashMap<&'a str, Vec<(String, Vec<u8>)>> {
    let mut map: HashMap<&str, Vec<(String, Vec<u8>)>> = HashMap::new();

    let it = archive
        .file_names()
        .map(|x| x.to_string())
        .collect::<Vec<String>>();

    // let main_dir = vec![];

    for d in sort {
        println!("make {}", d);
        map.insert(d, vec![]);
    }

    for file_name in it {
        let shorter = if file_name.starts_with("./") {
            &file_name[2..file_name.len()]
        } else {
            &file_name
        };

        let part = shorter.split("/").collect::<Vec<&str>>();
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
            loggy.log(
                LogType::Config,
                &format!(
                    "check file {} and convert to dir {} and name {}",
                    shorter, dir, name
                ),
            );

            loggy.log(
                LogType::Config,
                &format!("full {}, file {}, dir {}", file_name, name, dir),
            );
            // let t = archive.by_index(0).unwrap();
            match archive.by_name(&file_name) {
                Ok(mut file) => match map.get_mut(dir) {
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
                    loggy.log(LogType::ConfigError, &"problem reading archive");
                    println!("?");
                }
            };
            // }

            loggy.log(LogType::Config, &format!("list: {}", file_name));
        } else {
            loggy.log(LogType::ConfigError, &format!("bad path for {}", file_name));
        }
    }

    map
}

/** Unpacked a packed game image-zip into just the zip as a u8 buffer, buffer will still need unzipping */
pub fn unpack(gamefile: Vec<u8>, loggy: &mut Loggy) -> Vec<u8> {
    // let mut gamefile = get_file_buffer(target);
    if gamefile.len() <= 0 {
        loggy.log(LogType::ConfigError, &"file to unpack is 0 bytes!");
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
    loggy.log(LogType::Config, &format!("stretch size {}", v.len()));

    v
}

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

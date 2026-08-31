use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Cursor, Read, Write};
use std::path::{Component, PathBuf};
use std::{fs::File, path::Path};

// use zip::result::ZipError;
// use zip::write::FileOptions;

// use async_zip::base::read::mem::ZipFileReader;
// use async_zip::base::read::seek::ZipFileReader;
use async_zip::base::write::ZipFileWriter;
use async_zip::error::ZipError;
// use async_zip::tokio::read::ZipEntryReader;
use async_zip::tokio::read::seek::ZipFileReader;
use async_zip::ZipEntryBuilder;
// use futures::TryFutureExt;
// use futures_lite::io::Cursor;

use crate::error::P64Error;
use crate::log::{LogType, Loggy};
use tokio_util::compat::TokioAsyncReadCompatExt;

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
    let file = match File::create(&path) {
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
    Ok(match File::open(&path) {
        Ok(f) => {
            let size = f.metadata().map_err(|e| P64Error::IoError(e))?.len() as usize;
            let mut v = Vec::with_capacity(size);
            let mut reader = BufReader::new(f);

            match reader.read_to_end(&mut v) {
                Ok(_) => {
                    //log(format!("buffer size {} for {}", x, &path_str)),
                }
                Err(e) => {
                    return Err(P64Error::IoError(e));
                }
            };
            v
        }
        _ => vec![],
    })
}

/** Load file contents as utf8 string from path */
pub fn get_file_string_from_path(path: PathBuf) -> Result<String, P64Error> {
    let v = get_file_buffer_from_path(path)?;
    match String::from_utf8(v) {
        Ok(s) => Ok(s),
        Err(_e) => Err(P64Error::IoUtf8Error),
    }
}

/// Resolve a game-supplied path inside `dir`, refusing anything that could leave it.
///
/// Only *plain relative* components are allowed. Checking for `..` alone was not
/// enough: `Path::join` **discards the base** when the argument is absolute, so
/// `io.get("/etc/passwd")` produced `/etc/passwd` — it contains no `ParentDir`
/// component, so it passed the old check and read straight out of the sandbox. A
/// Windows `Prefix` (`C:`, `\\?\`, UNC) does the same thing.
///
/// Still trusts the filesystem not to point out of `dir` on our behalf: a symlink
/// inside the game folder is followed. Closing that means canonicalising, which is
/// awkward for writes to files that don't exist yet, so it's left as a known limit
/// rather than half-done.
fn scrub_path(dir: &str, path: &str) -> Result<PathBuf, P64Error> {
    let p = PathBuf::new().join(path);
    let ok = p
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if !ok || path.is_empty() {
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

/// Every file under `dir`, as slash-separated paths relative to it
/// (`scripts/main.lua`) and sorted, so an editor can list what it's allowed to open.
///
/// Symlinks are skipped rather than followed: a link planted inside an app folder
/// would otherwise read or overwrite anything on the machine, which is the same
/// escape [`scrub_path`] exists to close. Dotfiles are skipped too — `.git` in a
/// game folder is noise an editor shouldn't offer to edit.
pub fn list_files_scrubbed(dir: &str) -> Result<Vec<String>, P64Error> {
    let root = PathBuf::new().join(dir);
    let mut out = vec![];
    // Depth is bounded because a game folder is shallow by nature, and because
    // symlinks are skipped there's no cycle to guard against.
    let mut stack = vec![(root.clone(), String::new())];
    while let Some((path, prefix)) = stack.pop() {
        let entries = match std::fs::read_dir(&path) {
            Ok(e) => e,
            Err(e) => return Err(P64Error::IoError(e)),
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            let rel = if prefix.is_empty() {
                name
            } else {
                format!("{}/{}", prefix, name)
            };
            // `file_type` here comes from the directory entry, so it reports a
            // symlink as a symlink instead of what it points at.
            match entry.file_type() {
                Ok(t) if t.is_dir() => stack.push((entry.path(), rel)),
                Ok(t) if t.is_file() => out.push(rel),
                _ => {}
            }
        }
    }
    out.sort();
    Ok(out)
}

fn handle_zip_error(err: ZipError) -> P64Error {
    match err {
        ZipError::UpstreamReadError(i) => P64Error::IoError(i),
        _ => P64Error::IoInvalidArchive("unknown archive error"),
    }
}

// fn handle_zip_error(err: ZipError, f: Option<&str>) -> P64Error {
//     match err {
//         ZipError::Io(i) => P64Error::IoError(i),
//         ZipError::InvalidArchive(a) | zip::result::ZipError::UnsupportedArchive(a) => {
//             P64Error::IoInvalidArchive(a)
//         }
//         ZipError::FileNotFound => match f {
//             Some(ff) => P64Error::IoFileNotFound(ff.into()),
//             None => P64Error::IoFileNotFound("unknown".into()),
//         },
//         ZipError::InvalidPassword => P64Error::IoInvalidArchive("password zipped"),
//         ZipError::UnsupportedArchive(s) => P64Error::IoInvalidArchive(s),
//         _ => P64Error::IoInvalidArchive("unknown archive error"),
//     }
// }

/** read provided source string paths into a zip file, and smash it on to the end of an image file (see squish for simple smash) */
// pub fn pack_zip1(
//     sources: Vec<&str>,
//     thumb: PathBuf,
//     out: &str,
//     loggy: &mut Loggy,
// ) -> Result<(), P64Error> {
//     // let zipfile = std::fs::File::open(name).unwrap();
//     let mut bin = get_file_buffer_from_path(thumb)?;
//
//     if bin.is_empty() {
//         loggy.log(LogType::ConfigError,&"unable to pack file as icon chosen is not available, is it in the game directory root?");
//         return Err(P64Error::IoEmptyFile);
//     }
//     loggy.log(
//         LogType::Config,
//         &format!("using icon of {} bytes", bin.len()),
//     );
//
//     // let new_file = File::create(&Path::new("temp")).unwrap();
//     let v = Vec::new();
//     let c = Cursor::new(v);
//
//     let mut zip = zip::ZipWriter::new(c);
//     let options = FileOptions::<()>::default();
//     for source in sources {
//         if let Err(err) = zip.start_file(
//             source,
//             options.compression_method(zip::CompressionMethod::Stored),
//         ) {
//             return Err(handle_zip_error(err, Some(&source)));
//         }
//
//         let buff = get_file_buffer(&source)?;
//         let buffy = buff.as_slice();
//         if let Err(err) = zip.write(buffy) {
//             loggy.log(LogType::ConfigError, &format!("zipping error: {}", err));
//         }
//     }
//
//     match zip.finish() {
//         Ok(mut f) => {
//             f.set_position(0);
//
//             // Read the "file's" contents into a vector
//             let mut buf = Vec::new();
//             f.read_to_end(&mut buf).unwrap();
//             loggy.log(LogType::Config, &format!("zip buffer size {}", buf.len()));
//
//             bin.append(&mut buf);
//             let new_file = File::create(&Path::new(out)).unwrap();
//             let mut writer = BufWriter::new(new_file);
//             match writer.write(bin.as_slice()) {
//                 Ok(_) => {
//                     loggy.log(LogType::Config, &"cartridge zipped!");
//                     Ok(())
//                 }
//                 Err(err) => Err(P64Error::IoError(err)),
//             }
//         }
//         Err(e) => Err(handle_zip_error(e, Some(out))),
//     }
// }

/** read provided source string paths into a zip file, and smash it on to the end of an image file (see squish for simple smash) */
// Packing a game bundle writes a zip to disk via tokio::fs — there is no
// filesystem on the web, so the wasm build gets a stub that reports the
// operation as unsupported instead.
#[cfg(target_arch = "wasm32")]
pub async fn pack_zip(
    _sources: Vec<&str>,
    _thumb: PathBuf,
    _out: &str,
    _loggy: &mut Loggy,
) -> Result<(), P64Error> {
    Err(P64Error::IoError(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "bundle packing is not supported on the web",
    )))
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn pack_zip(
    sources: Vec<&str>,
    thumb: PathBuf,
    out: &str,
    loggy: &mut Loggy,
) -> Result<(), P64Error> {
    // use tokio::io::AsyncWriteExt;

    let bin = get_file_buffer_from_path(thumb)?;
    if bin.is_empty() {
        loggy.log(
            LogType::ConfigError,
            &"unable to pack file as icon chosen is not available, is it in the game directory root?",
        );
        return Err(P64Error::IoEmptyFile);
    }

    loggy.log(
        LogType::Config,
        &format!("using icon of {} bytes", bin.len()),
    );

    let new_file = tokio::fs::File::create(&Path::new(out)).await;

    let mut new_file = new_file.map_err(|e| P64Error::IoError(e))?;
    // Write the icon PNG first, then append the zip, so the output is a valid,
    // viewable .game.png (image + trailing zip). unpack() strips back to the
    // PNG's IEND to recover the zip. (This prepend was dropped in the async_zip
    // migration, which quietly turned carts into raw zips.)
    use tokio::io::AsyncWriteExt;
    new_file
        .write_all(&bin)
        .await
        .map_err(|e| P64Error::IoError(e))?;
    let mut writer = ZipFileWriter::with_tokio(&mut new_file);

    for source in sources {
        let builder = ZipEntryBuilder::new(source.into(), async_zip::Compression::Stored);
        // let entry=writer.write_entry_stream(builder).await.map_err(|e| handle_zip_error(e))?;

        let data = get_file_buffer(&source)?;
        writer.write_entry_whole(builder, &data).await?;
    }

    writer.close().await?;
    Ok(())
}

impl From<ZipError> for P64Error {
    fn from(e: ZipError) -> Self {
        handle_zip_error(e)
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

pub async fn get_archive(
    file: Vec<u8>,
    loggy: &mut Loggy,
) -> Result<ZipFileReader<Cursor<Vec<u8>>>, &'static str> {
    let v = unpack(file, loggy);
    if v.len() <= 0 {
        return Err("archive is empty");
    }
    let c = Cursor::new(v);

    ZipFileReader::new(c.compat())
        .await
        .map_err(|_| "failed to build archive")
    // let rr=reader.reader_without_entry(0).await.map_err(|_| "archive empty")?;
    //
    // // let rrr=(rr.compat());
}

/** unpack a packed game image-zip and load all assets into memory and return as asset-path keyed hashmap of u8 buffers  */
pub async fn unpack_and_walk<'a>(
    archive: &mut ZipFileReader<Cursor<Vec<u8>>>,
    sort: Vec<&'a str>,
    loggy: &mut Loggy,
) -> Result<HashMap<&'a str, Vec<(String, Vec<u8>)>>, &'static str> {
    let mut map: HashMap<&str, Vec<(String, Vec<u8>)>> = HashMap::new();

    for d in sort {
        println!("make {}", d); // TODO remove need for this
        map.insert(d, vec![]);
    }
    let entries: Result<Vec<(usize, String)>, &'static str> = archive
        .file()
        .entries()
        .iter()
        .enumerate()
        .map(|(id, entry)| {
            let file_name = entry
                .filename()
                .as_str()
                .map_err(|_| "non UTF8 zip asset")?
                .to_string();
            Ok((id, file_name))
        })
        .collect();
    let entries = entries?;

    for (id, file_name) in entries {
        let shorter = if file_name.starts_with("./") {
            &file_name[2..file_name.len()]
        } else {
            &file_name
        };
        let part = shorter.split("/").collect::<Vec<&str>>();
        if part.len() > 1 {
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

            match map.get_mut(dir) {
                Some(ar) => {
                    let mut contents: Vec<u8> = Vec::new();
                    // println!("found file");

                    let mut data_reader = archive
                        .reader_with_entry(id)
                        .await
                        .map_err(|_| "problem reading zip entry header")?;
                    // match file.read_to_end(&mut contents) {
                    //     Ok(_) => {}
                    //     _ => {}
                    // }
                    data_reader
                        .read_to_end_checked(&mut contents)
                        .await
                        .map_err(|_| "problem reading zip entry contents")?;
                    ar.push((shorter.to_owned(), contents));
                }
                _ => {}
            }
        }
    }
    Ok(map)
}

/** Unpacked a packed game image-zip into just the zip as a u8 buffer, buffer will still need unzipping */
pub fn unpack(gamefile: Vec<u8>, loggy: &mut Loggy) -> Vec<u8> {
    // let mut gamefile = get_file_buffer(target);
    if gamefile.len() <= 0 {
        loggy.log(LogType::ConfigError, &"file to unpack is 0 bytes!");
        return vec![];
    }

    // A bundle is either a raw zip (`PK\x03\x04`, what `pack` currently writes)
    // or a viewable .game.png with the zip appended after the PNG's IEND chunk.
    // Raw zip: hand it back untouched. PNG: strip up to IEND (the code below).
    // (Without this, the IEND scan finds the marker *inside* an embedded PNG
    // asset and returns a corrupted fragment — no entries load.)
    if gamefile.len() >= 4 && gamefile[..4] == [0x50, 0x4B, 0x03, 0x04] {
        loggy.log(LogType::Config, &format!("raw zip bundle, {} bytes", gamefile.len()));
        return gamefile;
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    /// A game's `io.get`/`io.set` path must stay inside its own folder. The absolute
    /// case is the one that actually escaped: it carries no `..`, so a traversal
    /// check alone waved it through while `Path::join` threw the sandbox root away.
    #[test]
    fn scrub_path_confines_to_dir() {
        let dir = "/games/mygame";
        assert_eq!(
            scrub_path(dir, "notes.txt").unwrap(),
            PathBuf::from("/games/mygame/notes.txt")
        );
        assert_eq!(
            scrub_path(dir, "sub/ok.txt").unwrap(),
            PathBuf::from("/games/mygame/sub/ok.txt")
        );
        assert_eq!(
            scrub_path(dir, "./here.txt").unwrap(),
            PathBuf::from("/games/mygame/./here.txt")
        );

        for bad in [
            "/etc/passwd",     // absolute: replaces the base entirely
            "../secret",       // classic traversal
            "sub/../../secret",// traversal after a valid component
            "",                // nothing to resolve
        ] {
            assert!(
                scrub_path(dir, bad).is_err(),
                "expected {:?} to be refused",
                bad
            );
        }
    }

    /// What an overlay's file list looks like. `test/target` is a fixture app kept
    /// deliberately tiny, so this also pins the shape of the paths handed to Lua:
    /// relative to the app folder, slash-separated, sorted, subfolders included.
    #[test]
    fn list_files_scrubbed_walks_and_relativizes() {
        let files = list_files_scrubbed("test/target").unwrap();
        assert_eq!(
            files,
            vec!["assets/example.png", "icon.png", "scripts/main.lua"]
        );
    }
}

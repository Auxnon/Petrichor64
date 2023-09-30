use std::fmt::{Display, Formatter};

pub enum P64Error {
    PermPathTraversal,
    IoError(std::io::Error),
    IoUtf8Error,
    IoInvalidArchive(&'static str),
    IoFileNotFound(Box<str>),
    IoNotFileOrDir(Box<str>),
    IoEmptyFile,
}

impl Display for P64Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            P64Error::PermPathTraversal => write!(f, "Permission denied: Path traversal"),
            P64Error::IoError(err) => write!(f, "IO Error: {}", err),
            P64Error::IoUtf8Error => write!(f, "IO Error: UTF-8 conversion"),
            P64Error::IoInvalidArchive(err) => write!(f, "IO Error: Invalid archive {}", err),
            P64Error::IoFileNotFound(fi) => write!(f, "IO Error: File {} not found", fi),
            P64Error::IoNotFileOrDir(fi) => {
                write!(f, "IO Error: {} is not a File or directory", fi)
            }
            P64Error::IoEmptyFile => write!(f, "IO Error: Empty file"),
        }
    }
}

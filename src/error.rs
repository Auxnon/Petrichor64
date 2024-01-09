use std::fmt::{Display, Formatter};

use piccolo::{PrototypeError, StaticError};

pub enum P64Error {
    PermPathTraversal,
    IoError(std::io::Error),
    IoUtf8Error,
    IoInvalidArchive(&'static str),
    IoFileNotFound(Box<str>),
    IoNotFileOrDir(Box<str>),
    IoEmptyFile,
    LuaParseError(std::io::Error),
    LuaCompileError(std::io::Error),
    MissingAssets,
    MissingScripts,
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
            P64Error::LuaParseError(err) => write!(f, "Lua Error: {}", err),
            P64Error::LuaCompileError(err) => write!(f, "Lua Error: {}", err),
            P64Error::MissingAssets => write!(f, "Missing app asset directory and contents"),
            P64Error::MissingScripts => write!(f, "Missing app script directory and contents"),
        }
    }
}

impl From<std::str::Utf8Error> for P64Error {
    fn from(value: std::str::Utf8Error) -> Self {
        P64Error::IoUtf8Error
    }
}

// impl From<ParserError> for P64Error {
//     fn from(value: ParserError) -> Self {
//         P64Error::IoError(std::io::Error::new(std::io::ErrorKind::Other, value))
//     }
// }

// impl From<CompilerError> for P64Error {
//     fn from(value: CompilerError) -> Self {
//         P64Error::IoError(std::io::Error::new(std::io::ErrorKind::Other, value))
//     }
// }

impl From<StaticError> for P64Error {
    fn from(value: StaticError) -> Self {
        P64Error::IoError(std::io::Error::new(std::io::ErrorKind::Other, value))
    }
}

impl From<PrototypeError> for P64Error {
    fn from(value: PrototypeError) -> Self {
        match value {
            PrototypeError::Parser(e) => {
                P64Error::LuaParseError(std::io::Error::new(std::io::ErrorKind::Other, e))
            }
            PrototypeError::Compiler(e) => {
                P64Error::LuaCompileError(std::io::Error::new(std::io::ErrorKind::Other, e))
            }
        }
    }
}

use std::{
    fmt::{Display, Formatter},
    sync::mpsc::SendError,
};

use silt_lua::error::ErrorTuple;

// use piccolo::{PrototypeError, StaticError};

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
    LuaRunError(Vec<ErrorTuple>),
    LuaGenericError,
    MissingAssets,
    MissingScripts,
    ChannelTimeoutError(u8),
    ChannelDisconnectedError,
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
            P64Error::ChannelTimeoutError(i) => write!(f, "Lua channel ({i}) timed out"),
            P64Error::ChannelDisconnectedError => write!(f, "Lua thread channel broken"),
            P64Error::LuaGenericError => write!(f, "Lua unknown failure occured"),
            P64Error::LuaRunError(err) => {
                let s = err.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ");
                write!(f, "{}", s)
            }
        }
    }
}

impl From<std::str::Utf8Error> for P64Error {
    fn from(value: std::str::Utf8Error) -> Self {
        P64Error::IoUtf8Error
    }
}

impl<T> From<SendError<T>> for P64Error {
    fn from(_: SendError<T>) -> Self {
        P64Error::ChannelDisconnectedError
    }
}

impl From<Vec<ErrorTuple>> for P64Error {
    fn from(value: Vec<ErrorTuple>) -> Self {
        P64Error::LuaRunError(value)
    }
}

impl std::fmt::Debug for P64Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self)
    }
}

impl std::error::Error for P64Error {}

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

// impl From<StaticError> for P64Error {
//     fn from(value: LuaError ) -> Self {
//         P64Error::IoError(std::io::Error::new(std::io::ErrorKind::Other, value))
//     }
// }

// impl From<PrototypeError> for P64Error {
//     fn from(value: LuaError) -> Self {
//         match value {
//             LuaError::VmCompileError
//             LuaError::VmCompileError
//             PrototypeError::Parser(e) => {
//                 P64Error::LuaParseError(std::io::Error::new(std::io::ErrorKind::Other, e))
//             }
//             PrototypeError::Compiler(e) => {
//                 P64Error::LuaCompileError(std::io::Error::new(std::io::ErrorKind::Other, e))
//             }
//         }
//     }
// }

use std::{
    fmt::{Display, Formatter},
    sync::mpsc::SendError,
};

use silt_lua::{error::ErrorTuple, LuaError};

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
    LuaRunError(Box<ErrorTuple>),
    LuaGenericError,
    MissingAssets,
    MissingScripts,
    ChannelTimeoutError,
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
            P64Error::ChannelTimeoutError => write!(f, "Lua channel timed out"),
            P64Error::ChannelDisconnectedError => write!(f, "Lua thread channel broken"),
            P64Error::LuaGenericError => write!(f, "Lua unknown failure occured"),
            P64Error::LuaRunError(err) => {
                // writeln!("\n❌ Lua Parse Errors:\n");
                // for (i, err) in (*error_tuple).iter().enumerate() {
                //     writeln!(
                //         "  [{}] {}:{} - {}",
                //         i + 1,
                //         err.location.0, err.location.1, err.code
                //     );
                // }
                // writeln!("\nFound {} error(s)\n", errors.len());
                // Ok(())

                write!(f, "  {}:{} - {}", err.location.0, err.location.1, err.code)
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
    fn from(mut value: Vec<ErrorTuple>) -> Self {
        let n = value.len();
        if n == 0 {
            return P64Error::LuaGenericError;
        } else if n == 1 {
            return P64Error::LuaRunError(Box::new(value.pop().unwrap()));
        }
        P64Error::LuaRunError(Box::new(value.swap_remove(0)))
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

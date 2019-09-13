use std::fmt::{self, Display, Debug};
use std::io;
use std::path::PathBuf;

pub enum Error {
    Git2(git2::Error),
    JsonLine(PathBuf, serde_json::Error),
    Json(serde_json::Error),
    Io(io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match self {
            Git2(e) => write!(f, "{}", e),
            JsonLine(path, e) => write!(f, "{}: {}", path.display(), e),
            Json(e) => write!(f, "{}", e),
            Io(e) => write!(f, "{}", e),
        }
    }
}

impl Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match self {
            Git2(e) => write!(f, "{}", e),
            JsonLine(path, e) => write!(f, "{}: {}", path.display(), e),
            Json(e) => write!(f, "{}", e),
            Io(e) => write!(f, "{}", e),
        }
    }
}

impl From<git2::Error> for Error {
    fn from(err: git2::Error) -> Self {
        Error::Git2(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

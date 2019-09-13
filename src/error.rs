use semver::ReqParseError;

use std::fmt::{self, Display, Debug};
use std::io;

pub enum Error {
    MissingJson,
    ParseSeries(String, ReqParseError),
    Io(io::Error),
    Json(serde_json::Error),
    Reqwest(reqwest::Error),
    Regex(regex::Error),
    NothingFound,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Error::*;

        match self {
            MissingJson => write!(
                f,
                "missing ./{}; run `cargo tally --init`",
                cargo_tally::JSONFILE
            ),
            ParseSeries(s, err) => write!(f, "failed to parse series {}: {}", s, err),
            Io(err) => write!(f, "{}", err),
            Json(err) => write!(f, "{}", err),
            Reqwest(err) => write!(f, "{}", err),
            Regex(err) => write!(f, "{}", err),
            NothingFound => write!(f, "nothing found for this crate"),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<serde_json::Error> for Error {
    fn from(err: serde_json::Error) -> Self {
        Error::Json(err)
    }
}

impl From<reqwest::Error> for Error {
    fn from(err: reqwest::Error) -> Self {
        Error::Reqwest(err)
    }
}

impl From<regex::Error> for Error {
    fn from(err: regex::Error) -> Self {
        Error::Regex(err)
    }
}

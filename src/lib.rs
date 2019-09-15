use chrono::Utc;
use fnv::{FnvHashMap as Map, FnvHashSet as Set};
use semver::{Version, VersionReq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::fmt::{self, Display};

pub const JSONFILE: &str = "tally.json.gz";
pub const COMPFILE: &str = "computed.json.gz";

pub type DateTime = chrono::DateTime<Utc>;

#[derive(Serialize, Deserialize)]
pub struct Crate {
    pub published: Option<DateTime>,
    pub name: String,
    #[serde(rename = "vers")]
    pub version: Version,
    #[serde(rename = "deps")]
    pub dependencies: Vec<Dependency>,
    pub features: Map<String, Vec<Feature>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    pub req: VersionReq,
    pub features: Vec<String>,
    pub optional: bool,
    pub default_features: bool,
    #[serde(default, deserialize_with = "null_as_default")]
    pub kind: DependencyKind,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranitiveDep {
    pub name: String,
    pub timestamp: DateTime,
    pub version: Version,
    pub transitive_count: usize,
    pub direct_count: usize,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DependencyKind {
    Normal,
    Build,
    Dev,
}

#[derive(Clone, Debug)]
pub enum Feature {
    Current(String),
    Dependency(String, String),
}

impl Default for DependencyKind {
    fn default() -> Self {
        DependencyKind::Normal
    }
}

fn null_as_default<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Deserialize<'de> + Default,
    D: Deserializer<'de>,
{
    let option = Option::deserialize(deserializer)?;
    Ok(option.unwrap_or_default())
}

impl Display for DependencyKind {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let string = match self {
            DependencyKind::Normal => "normal",
            DependencyKind::Build => "build",
            DependencyKind::Dev => "dev",
        };
        write!(f, "{}", string)
    }
}

impl Display for Feature {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Feature::Current(feat) => write!(f, "{}", feat),
            Feature::Dependency(dep, feat) => write!(f, "{}/{}", dep, feat),
        }
    }
}

impl Serialize for Feature {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Feature {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let mut s = String::deserialize(deserializer)?;
        Ok(match s.find('/') {
            Some(slash) => {
                let feature = s[slash + 1..].to_owned();
                s.truncate(slash);
                Feature::Dependency(s, feature)
            }
            None => Feature::Current(s),
        })
    }
}

mod error {
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
                    super::JSONFILE
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
}

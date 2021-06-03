mod compat;

use chrono::Utc;
use fnv::FnvHashMap as Map;
use semver::{Version, VersionReq};
use serde::de::Visitor;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::fmt::{self, Display};

pub const JSONFILE: &str = "tally.json.gz";

pub type DateTime = chrono::DateTime<Utc>;

#[derive(Serialize, Deserialize)]
pub struct Crate {
    pub published: Option<DateTime>,
    pub name: String,
    #[serde(rename = "vers", deserialize_with = "version_compat")]
    pub version: Version,
    #[serde(rename = "deps")]
    pub dependencies: Vec<Dependency>,
    pub features: Map<String, Vec<Feature>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dependency {
    pub name: String,
    #[serde(deserialize_with = "version_req_compat")]
    pub req: VersionReq,
    pub features: Vec<String>,
    pub optional: bool,
    pub default_features: bool,
    #[serde(default, deserialize_with = "null_as_default")]
    pub kind: DependencyKind,
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
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
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

fn version_compat<'de, D>(deserializer: D) -> Result<Version, D::Error>
where
    D: Deserializer<'de>,
{
    struct VersionVisitor;

    impl<'de> Visitor<'de> for VersionVisitor {
        type Value = Version;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("semver version")
        }

        fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            compat::version(string).map_err(serde::de::Error::custom)
        }
    }

    deserializer.deserialize_str(VersionVisitor)
}

fn version_req_compat<'de, D>(deserializer: D) -> Result<VersionReq, D::Error>
where
    D: Deserializer<'de>,
{
    struct VersionReqVisitor;

    impl<'de> Visitor<'de> for VersionReqVisitor {
        type Value = VersionReq;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("semver version req")
        }

        fn visit_str<E>(self, string: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            compat::version_req(string).map_err(serde::de::Error::custom)
        }
    }

    deserializer.deserialize_str(VersionReqVisitor)
}

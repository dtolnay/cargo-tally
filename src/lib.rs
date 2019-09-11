use chrono::Utc;
use fnv::FnvHashMap as Map;
use semver::{Version, VersionReq};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use std::fmt::{self, Display};

pub const JSONFILE: &str = "tally.json.gz";
pub const COMPFILE: &str = "comp.json.gz";

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

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CrateMeta {
    name: String,
    version: VersionReq,
    // TODO do i need this info??
    features: Vec<String>,
    kind: DependencyKind,
}

impl CrateMeta {
    fn new(dep: &Dependency) -> Self {
        // TODO try not to clone everything TransitiveCrateDeps too !!!
        Self {
            name: dep.name.to_string(),
            version: dep.req.clone(),
            features: dep.features.clone(),
            kind: dep.kind.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranitiveCrateDeps {
    pub name: String,
    pub version: Version,
    pub features: Map<String, Vec<Feature>>,
    /// Crates that depend on this crate
    pub depended_on: Vec<CrateMeta>,
    // or we could just use the count
    // count: usize,
}

impl TranitiveCrateDeps {
    fn collect_deps(crates: &[Crate], search_crate: &Crate) -> Vec<CrateMeta> {
        let mut res = Vec::new();
        for krate in crates {
            if let Some(dep) = krate.dependencies.iter().find(|dep| {
                // collect all crates that depend on `search_crate`, non optional and non dev dep
                dep.name == search_crate.name && !dep.optional && dep.kind != DependencyKind::Dev
            }) {
                res.push(CrateMeta::new(dep));
            }
        }
        res
    }
    pub fn calc_dependencies(crates: &[Crate], krate: &Crate) -> Self {
        let depended_on = Self::collect_deps(crates, krate);
        Self {
            name: krate.name.to_owned(),
            version: krate.version.clone(),
            features: krate.features.clone(),
            depended_on,
        }
    }
    pub fn calculate_trans_deps(&self) {
        // calculate from Vec<Crate>
    }
}

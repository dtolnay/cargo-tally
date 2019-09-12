use chrono::Utc;
use fnv::{FnvHashMap as Map, FnvHashSet as Set};
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


use self::intern::CrateName;

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub(crate) struct CrateKey {
    pub(crate) name: CrateName,
    pub(crate) index: u32,
}

#[derive(Clone, Debug)]
pub(crate) struct Metadata {
    pub(crate) num: Version,
    created_at: DateTime,
    features: Map<String, Vec<Feature>>,
    dependencies: Vec<Dependency>,
}

#[derive(Debug)]
pub(crate) struct Universe {
    pub(crate) crates: Map<CrateName, Vec<Metadata>>,
    depends: Map<CrateKey, Vec<CrateKey>>,
    reverse_depends: Map<CrateKey, Set<CrateKey>>,
    transitive: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DepCrateMeta {
    name: String,
    parent: String,
    version: VersionReq,
    // TODO do i need this info??
    features: Vec<String>,
    kind: DependencyKind,
}

impl DepCrateMeta {
    fn new(dep: &Dependency, parent: &str) -> Self {
        // TODO try not to clone everything TransitiveCrateDeps too !!!
        Self {
            name: dep.name.to_string(),
            parent: parent.to_string(),
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
    pub depended_on: Vec<DepCrateMeta>,
    // or we could just use the count
    // count: usize,
}

impl TranitiveCrateDeps {
    fn collect_deps(crates: &[Crate], search: &Crate, ret: &mut Vec<DepCrateMeta>) {
        // TODO recursive ????? do i need to go deeper to actually hit all 
        // transitive deps
        // TODO use HashMap trick to speed up indexing??
        
        // finds crates that directly depend
        for krate in crates {
            for dep in krate.dependencies.iter() {
                // collect all crates that depend on `search_crate`, non optional and non dev dep
                if dep.name == search.name && !dep.optional && dep.kind != DependencyKind::Dev {
                    let meta = DepCrateMeta::new(dep, &krate.name);
                    //println!("{:#?}", meta);
                    ret.push(meta);
                }
                // search dependencies of every crate for matches to `search`
                if let Some(t_crate) = crates.iter().find(|t_crate| t_crate.name == dep.name) {
                    let found = t_crate.dependencies.iter().find(|&t_dep| {
                        t_dep.name == search.name && !t_dep.optional && t_dep.kind != DependencyKind::Dev
                    });
                    if let Some(t_dep) = found {
                        let meta = DepCrateMeta::new(t_dep, &t_crate.name);
                        //println!("{:#?}", meta);
                        ret.push(meta);
                    }
                }
            }
        }

        //let mut ret = Vec::new();
        //ret
    }

    pub fn calc_dependencies(crates: &[Crate], krate: &Crate) -> Self {
        let mut depended_on = Vec::new();
        Self::collect_deps(crates, krate, &mut depended_on);
        // println!("{:?}", depended_on);
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

// TODO figure how to split into files
mod intern {
    use fnv::FnvBuildHasher;
    use lazy_static::lazy_static;
    use string_interner::{StringInterner, Symbol};

    use std::cell::UnsafeCell;
    use std::fmt::{self, Debug, Display};
    use std::marker::PhantomData;
    use std::ops::Deref;

    lazy_static! {
        static ref INTERN: SymbolsStayOnOneThread = Default::default();
    }

    struct SymbolsStayOnOneThread {
        interner: UnsafeCell<StringInterner<CrateName, FnvBuildHasher>>,
    }

    impl Default for SymbolsStayOnOneThread {
        fn default() -> Self {
            SymbolsStayOnOneThread {
                interner: UnsafeCell::new(StringInterner::with_hasher(Default::default())),
            }
        }
    }

    unsafe impl Send for SymbolsStayOnOneThread {}
    unsafe impl Sync for SymbolsStayOnOneThread {}

    #[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
    pub struct CrateName {
        value: u32,
        not_send_sync: PhantomData<*const ()>,
    }

    pub fn crate_name<T: Into<String> + AsRef<str>>(string: T) -> CrateName {
        let c = INTERN.interner.get();
        let c = unsafe { &mut *c };
        c.get_or_intern(string)
    }

    impl Symbol for CrateName {
        fn from_usize(value: usize) -> Self {
            CrateName {
                value: value as u32,
                not_send_sync: PhantomData,
            }
        }

        fn to_usize(self) -> usize {
            self.value as usize
        }
    }

    impl Deref for CrateName {
        type Target = str;

        fn deref(&self) -> &str {
            let c = INTERN.interner.get();
            unsafe {
                let c = &*c;
                c.resolve_unchecked(*self)
            }
        }
    }

    impl Debug for CrateName {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            Debug::fmt(&**self, formatter)
        }
    }

    impl Display for CrateName {
        fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            Display::fmt(&**self, formatter)
        }
    }

    impl<'a> PartialEq<&'a str> for CrateName {
        fn eq(&self, other: &&str) -> bool {
            &**self == *other
        }
    }
}

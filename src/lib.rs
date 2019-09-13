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


use self::intern::{crate_name, CrateName};

use indicatif::ProgressBar;
use log::{debug, info};
use semver_parser::range::{self, Op::Compatible, Predicate};

use std::u64;

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug, Serialize, Deserialize)]
pub struct CrateKey {
    pub name: CrateName,
    pub index: u32,
}
impl CrateKey {
    fn new(name: CrateName, index: u32) -> Self {
        CrateKey { name, index }
    }
}
// impl Serialize for CrateKey {
//     fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
//     where
//         S: Serializer,
//     {
//         serializer.collect_str(&format!("{:?}", self.name))
//     }
// }
// impl<'de> Deserialize<'de> for CrateKey {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: Deserializer<'de>,
//     {
//         let mut s = String::deserialize(deserializer)?;
//         Ok(match s.find('/') {
//             Some(slash) => {
//                 let feature = s[slash + 1..].to_owned();
//                 s.truncate(slash);
//                 Feature::Dependency(s, feature)
//             }
//             None => Feature::Current(s),
//         })
//     }
// }

#[derive(Debug)]
struct Matcher {
    name: CrateName,
    req: VersionReq,
    nodes: Vec<u32>,
}

#[derive(Debug)]
struct Event {
    name: CrateName,
    num: Version,
    timestamp: DateTime,
    features: Map<String, Vec<Feature>>,
    dependencies: Vec<Dependency>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub(crate) num: Version,
    created_at: DateTime,
    features: Map<String, Vec<Feature>>,
    dependencies: Vec<Dependency>,
}

#[derive(Debug)]
pub struct Universe {
    pub(crate) crates: Map<CrateName, Vec<Metadata>>,
    pub depends: Map<CrateKey, Vec<CrateKey>>,
    pub reverse_depends: Map<CrateKey, Set<CrateKey>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Row {
    pub timestamp: DateTime,
    pub name: CrateName,
    pub num: Version,
    pub deps: Vec<Metadata>,
    // TODO what to do about Vec<usize> remove for now just usize
    pub counts: usize,
    pub total: usize,
}

impl Universe {
    fn new() -> Self {
        Universe {
            crates: Map::default(),
            depends: Map::default(),
            reverse_depends: Map::default(),
        }
    }

    fn process_event(&mut self, event: Event) {
            // TODO update redos and pin version nums

            info!("processing event {} {}", event.name, event.num);

            let mut redo = Set::default();
            if let Some(prev) = self.crates.get(&event.name) {
                let key = CrateKey::new(event.name, prev.len() as u32 - 1);
                for dep in &self.depends[&key] {
                    self.reverse_depends.get_mut(dep).unwrap().remove(&key);
                }
                self.depends.remove(&key);
                for (i, metadata) in prev.iter().enumerate() {
                    if compatible_req(&metadata.num).matches(&event.num) {
                        let key = CrateKey::new(event.name, i as u32);
                        for node in self.reverse_depends[&key].clone() {
                            for dep in &self.depends[&node] {
                                self.reverse_depends.get_mut(dep).unwrap().remove(&node);
                            }
                            redo.insert(node);
                        }
                    }
                }
            }

            // Fix up silly wildcard deps by pinning them to versions compatible
            // with the latest release of the dep
            let mut dependencies = event.dependencies;
            let version_max = Version::new(u64::MAX, u64::MAX, u64::MAX);
            for dep in &mut dependencies {
                if dep.req.matches(&version_max) {
                    let name = crate_name(&*dep.name);
                    if let Some(releases) = self.crates.get(&name) {
                        dep.req = compatible_req(&releases.last().unwrap().num);
                    }
                }
            }                 
            let metadata = Metadata {
                num: event.num,
                created_at: event.timestamp,
                features: event.features,
                dependencies,
            };

            // index is which dependency we mean in terms of Metadata
            let index = self.crates.entry(event.name).or_insert_with(Vec::new).len();
            let key = CrateKey::new(event.name, index as u32);

            self.resolve_and_add_to_graph(key, &metadata);

            self.reverse_depends.insert(key, Set::default());
            self.crates.get_mut(&event.name).unwrap().push(metadata);

            for outdated in redo {
                let metadata = self.crates[&outdated.name][outdated.index as usize].clone();
                debug!("re-resolving {} {}", outdated.name, metadata.num);
                self.resolve_and_add_to_graph(outdated, &metadata);
            }
    }

    fn resolve(&self, name: &str, req: &VersionReq) -> Option<u32> {
        self.crates
            .get(&crate_name(name))?
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, metadata)| req.matches(&metadata.num))
            .map(|(i, _)| i as u32)
    }

    fn resolve_and_add_to_graph(&mut self, key: CrateKey, metadata: &Metadata) {
        let mut resolve = Resolve { crates: Map::default(), };

        for dep in metadata.dependencies.iter() {
            if let Some(index) = self.resolve(&dep.name, &dep.req) {
                let name = crate_name(&dep.name);
                let key = CrateKey { name, index, };
                // RECURSIVELY walk deps of deps ect.
                resolve.add_crate(self, key, dep.default_features, &dep.features);
            }
        }

        for dep in resolve.crates.keys() {
            self.reverse_depends
                .entry(*dep)
                .or_insert_with(Set::default)
                .insert(key);
        }
        self.depends
            .insert(key, resolve.crates.keys().cloned().collect());
    }

    fn compute_counts(
        &self,
        timestamp: DateTime,
        name: CrateName,
        num: Version,
        deps: Vec<Metadata>,
        matcher: &Matcher,
    ) -> Row {
        let mut set = Set::default();
        Row {
            timestamp,
            name,
            num,
            deps,
            counts: {
                    set.clear();
                    for index in matcher.nodes.iter() {
                        let key = CrateKey::new(matcher.name, *index);
                        set.extend(self.reverse_depends[&key].iter().map(|key| key.name));
                    }
                    set.len()
                },
            total: self.crates.len(),
        }
    }
}

#[derive(Debug)]
struct Resolve {
    crates: Map<CrateKey, ResolvedCrate>,
}

#[derive(Debug)]
struct ResolvedCrate {
    features: Set<String>,
    resolved: Vec<Option<u32>>,
}

impl ResolvedCrate {
    fn no_resolve() -> Self {
        ResolvedCrate {
            features: Set::default(),
            resolved: Vec::new(),
        }
    }
}
impl Resolve {
    #![allow(clippy::map_entry)]
    fn add_crate(
        &mut self,
        universe: &Universe,
        key: CrateKey,
        default_features: bool,
        features: &[String],
    ) {
        let metadata = &universe.crates[&key.name][key.index as usize];

        debug!("adding crate {} {}", key.name, metadata.num);
        if !self.crates.contains_key(&key) {
            let resolved = metadata
                .dependencies
                .iter()
                .map(|dep| universe.resolve(&dep.name, &dep.req))
                .collect::<Vec<_>>();
            self.crates.insert(
                key,
                ResolvedCrate {
                    features: Set::default(),
                    resolved: resolved.clone(),
                },
            );
            for (dep, index) in metadata.dependencies.iter().zip(resolved) {
                if !dep.optional && dep.kind != DependencyKind::Dev && index.is_some() {
                    let key = CrateKey::new(crate_name(&*dep.name), index.unwrap());
                    self.add_crate(universe, key, dep.default_features, &dep.features);
                }
            }
        }

        if default_features {
            if let Some(default_features) = metadata.features.get("default") {
                for feature in default_features {
                    self.add_dep_or_crate_feature(universe, key, feature);
                }
            }
        }
        for feature in features {
            self.add_crate_feature(universe, key, feature);
        }
    }

    fn add_dep_or_crate_feature(&mut self, universe: &Universe, key: CrateKey, feature: &Feature) {
        let metadata = &universe.crates[&key.name][key.index as usize];

        debug!(
            "adding dep or feature {} {}:{}",
            key.name, metadata.num, feature
        );

        match *feature {
            Feature::Current(ref feature) => {
                self.add_crate_feature(universe, key, feature);
            }
            Feature::Dependency(ref name, ref feature) => {
                for (i, dep) in metadata.dependencies.iter().enumerate() {
                    if dep.name == *name {
                        if !self.crates.contains_key(&key) {
                            println!("uh-oh");
                        }
                        if let Some(resolved) = self.crates[&key].resolved[i] {
                            let key = CrateKey::new(crate_name(&**name), resolved);
                            self.add_crate(universe, key, dep.default_features, &dep.features);
                            self.add_crate_feature(universe, key, feature);
                        }
                        return;
                    }
                }
                // FIXME https://github.com/dtolnay/cargo-tally/issues/22
                /*
                panic!(
                    "feature not found: {} {}:{}/{}",
                    key.name, metadata.num, name, feature
                );
                */
            }
        }
    }

    fn add_crate_feature(&mut self, universe: &Universe, key: CrateKey, feature: &str) {
        let metadata = &universe.crates[&key.name][key.index as usize];

        if !self
            .crates
            .get_mut(&key)
            .unwrap()
            .features
            .insert(feature.to_owned())
        {
            return;
        }

        debug!("adding feature {} {}:{}", key.name, metadata.num, feature);

        if let Some(subfeatures) = metadata.features.get(feature) {
            for subfeature in subfeatures {
                self.add_dep_or_crate_feature(universe, key, subfeature);
            }
        } else {
            for (i, dep) in metadata.dependencies.iter().enumerate() {
                if dep.name == feature {
                    if !self.crates.contains_key(&key) {
                        println!("uh-oh");
                    }
                    if let Some(resolved) = self.crates[&key].resolved[i] {
                        let key = CrateKey::new(crate_name(feature), resolved);
                        self.add_crate(universe, key, dep.default_features, &dep.features);
                    }
                    return;
                }
            }
            if key.name == "libc" && metadata.num == Version::new(0, 2, 4) && feature == "no-std" {
                // Looks like a one-time glitch, jemalloc-sys 0.1.0 depends on
                // this nonexistent feature of libc 0.2.4.
                return;
            }
            // We get here if someone made a breaking change by removing a feature :(
        }
    }
}

fn create_matcher(krate: &str) -> Matcher {
    // TODO clean up move Error and Result to right place when ready
    // use self::error::Error;
    let mut pieces = krate.splitn(2, ':');
    Matcher {
        name: crate_name(pieces.next().unwrap()),
        req: match pieces.next().unwrap_or("*").parse() {
            Ok(req) => req,
            Err(err) => panic!("{:?}", err),
        },
        nodes: Vec::new(),
    }
}

fn compatible_req(version: &Version) -> VersionReq {
    use semver::Identifier as SemverId;
    use semver_parser::version::Identifier as ParseId;
    VersionReq::from(range::VersionReq {
        predicates: vec![Predicate {
            op: Compatible,
            major: version.major,
            minor: Some(version.minor),
            patch: Some(version.patch),
            pre: version
                .pre
                .iter()
                .map(|pre| match *pre {
                    SemverId::Numeric(n) => ParseId::Numeric(n),
                    SemverId::AlphaNumeric(ref s) => ParseId::AlphaNumeric(s.clone()),
                })
                .collect(),
        }],
    })
}

pub fn universe(crates: Vec<Crate>, pb: &ProgressBar) -> (Universe, Vec<Row>) {
    let mut universe = Universe::new();
    let mut table = Vec::new();
    for krate in crates {
    //if let Some(krate) = crates.into_iter().find(|k| k.name == "tar") {
        pb.inc(1);

        let name = crate_name(&krate.name);
        let timestamp = krate.published.unwrap(); 
        
        let mut matcher = create_matcher(&krate.name);

        let ev = Event {
            name,
            num: krate.version.clone(),
            timestamp,
            features: krate.features,
            dependencies: krate.dependencies,
        };

        universe.process_event(ev);

        if matcher.name == name && matcher.req.matches(&krate.version) {
            matcher.nodes.push(universe.crates[&name].len() as u32 - 1);
        }

        let deps = universe.crates[&name].clone();
        let row = universe.compute_counts(timestamp, name, krate.version, deps, &matcher);
        table.push(row);
    }
    (universe, table)
}

// TODO figure how to split into files
pub mod intern {
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

    #[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, super::Serialize, super::Deserialize)]
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

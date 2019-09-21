mod intern;

use cargo_tally::{Dependency, DependencyKind, Feature, DateTime, TranitiveDep};
use fnv::{FnvHashMap as Map, FnvHashSet as Set};
use indicatif::ProgressBar;
use log::{debug, info, warn, error};
use serde::{Deserialize, Serialize};
use semver::{Version, VersionReq};
use semver_parser::range::{self, Op::Compatible, Predicate};

use std::u64;

pub use cargo_tally::Crate;
pub use intern::{crate_name, CrateName};

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
    pub dir_depends: Map<CrateName, Set<CrateKey>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Row {
    pub timestamp: DateTime,
    pub name: CrateName,
    pub num: Version,
    pub deps: Vec<Metadata>,
    // TODO what to do about Vec<usize> remove for now just usize
    pub tran_counts: usize,
    pub dir_counts: usize,
    pub total: usize,
}

impl Universe {
    fn new() -> Self {
        Universe {
            crates: Map::default(),
            depends: Map::default(),
            reverse_depends: Map::default(),
            dir_depends: Map::default(),
        }
    }

    #[allow(clippy::cognitive_complexity)]
    fn process_event(&mut self, event: Event) {
        debug!("processing event {} {}", event.name, event.num);

        let mut redo = Set::default();
        if let Some(prev) = self.crates.get(&event.name) {
            // events CrateName and index into Metadata in Universe.crates
            let key = CrateKey::new(event.name, prev.len() as u32 - 1);
            for dep in &self.depends[&key] {
                self.reverse_depends.get_mut(dep).unwrap().remove(&key);
            }
            self.depends.remove(&key);
            for (i, metadata) in prev.iter().enumerate() {
                // when we find the metadata about this event version
                if compatible_req(&metadata.num).matches(&event.num) {
                    let key = CrateKey::new(event.name, i as u32);

                    for node in self.reverse_depends[&key].clone() {
                        for dep in &self.depends[&node] {
                            //if event.name == "tar" { info!("key= {:?} node= {:?} dep= {:?}", key, node, dep) };
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

    fn  resolve_and_add_to_graph(&mut self, key: CrateKey, metadata: &Metadata) {
        let mut d_resolve = Resolve { crates: Map::default(), };
        let mut t_resolve = Resolve { crates: Map::default(), };

        for dep in metadata.dependencies.iter() {

            // println!("DEP {:?}", key);

            // if the crate is in cratesio at the right version number
            if let Some(index) = self.resolve(&dep.name, &dep.req) {

                // println!("RESOLVED {} {}", dep.name, index);

                let name = crate_name(&dep.name);
                let key = CrateKey { name, index, };
                // direct deps just match and insert
                d_resolve.crates.insert(key, ResolvedCrate::no_resolve());
                // transitive dependencies RECURSIVELY walk deps of deps ect.
                t_resolve.add_crate(self, key, dep.default_features, &dep.features);
            }
        }

        // add `CrateKey`s of resolved crates, walks the graph and checks features 
        // transitive deps
        for dep in t_resolve.crates.keys() {
            self.reverse_depends
                .entry(*dep)
                .or_insert_with(Set::default)
                .insert(key);
        }
        // calculate direct deps too
        for dep in d_resolve.crates.keys() {
            self.dir_depends
                .entry(dep.name)
                .or_insert_with(Set::default)
                .insert(key);
        }
        
        self.depends
            .insert(key, t_resolve.crates.keys().cloned().collect());
    }

    fn extend_timeline(&mut self, krate: &CrateName, time: &DateTime) {
        let mut resolve = Resolve { crates: Map::default(), };

        let index = self.crates.get(krate).unwrap().len();
        let prev_key = CrateKey::new(*krate, (index - 1) as u32);
        let key = CrateKey::new(*krate, index as u32);

        let prev_meta = &self.crates[&prev_key.name][prev_key.index as usize];

        // find all rev deps for krate, iter each meta cmp `time` DateTime
        // if == then add new key and meta event for that crate
        
        if key.name == "tar" { warn!("{:?}", prev_meta) };
        if key.name == "tar" { warn!("{:?} {}", krate, time) };
        // warn!("{:?} {}", krate, time);
        for rev_dep in self.reverse_depends[&prev_key].iter() {
            // error!("{:?}", rev_dep);
            if key.name == "tar" { error!("{:?}", rev_dep) };

        }
        // info!("\n");
        if key.name == "tar" { info!("\n") };
        for dep in resolve.crates.keys() {
            self.reverse_depends
                .entry(*dep)
                .or_insert_with(Set::default)
                .insert(key);
        }
    }

    fn compute_counts(
        &self,
        timestamp: DateTime,
        name: CrateName,
        num: Version,
        deps: Vec<Metadata>,
        index: u32,
    ) -> Row {
        let mut set = Set::default();
        Row {
            timestamp,
            name,
            num,
            deps,
            tran_counts: {
                    set.clear();
                    let key = CrateKey::new(name, index);
                    set.extend(self.reverse_depends[&key].iter().map(|key| key.name));
                    set.len()
                },
            dir_counts: {
                    set.clear();
                    if let Some(deps) = self.dir_depends.get(&name) {
                        set.extend(deps.iter().map(|key| key.name));
                        set.len()
                    } else {
                        0
                    }
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

        if key.name == "tar" { info!("adding crate {} {}", key.name, metadata.num) };
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
                    let key2 = CrateKey::new(crate_name(&*dep.name), index.unwrap());
                    if key.name == "tar" { warn!("KEY {:?} DEP {:?}", key, dep) };
                    self.add_crate(universe, key2, dep.default_features, &dep.features);
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

pub fn pre_compute_graph(crates: Vec<Crate>, pb: &ProgressBar) -> Vec<TranitiveDep> {
    let mut universe = Universe::new();
    let mut table = Vec::new();
    for krate in crates {
        pb.inc(1);

        let name = crate_name(&krate.name);
        let timestamp = krate.published.unwrap();
        let ver = krate.version.clone();
        
        let ev = Event {
            name,
            num: krate.version.clone(),
            timestamp,
            features: krate.features,
            dependencies: krate.dependencies,
        };

        universe.process_event(ev);

        let deps = universe.crates[&name].clone();
        let idx = universe.crates[&name].len() as u32 - 1;
        let row = universe.compute_counts(timestamp, name, ver, deps, idx);

        table.push(TranitiveDep {
            name: krate.name,
            timestamp,
            version: krate.version,
            transitive_count: row.tran_counts,
            direct_count: row.dir_counts,
            total: row.total,
        });
    }

    use std::collections::BTreeSet;
    // TODO compute every changed depended on crate at every timestamp
    let mut time_set: BTreeSet<_> = universe.crates
        .iter()
        .flat_map(|(_, meta)| meta.iter().map(|m| m.created_at))
        .collect::<Set<_>>()
        .iter()
        .cloned()
        .collect();

    let mut time_vec = time_set
        .iter()
        .cloned()
        .collect::<Vec<_>>();

    time_vec.sort();
    time_vec.dedup();
    // TODO fix needing to do this to avoid immut/mut borrow
    let krates: Vec<(CrateName, Vec<Metadata>)> = universe.crates
        .iter()
        .map(|(n, metas)| (*n, metas.clone()))
        .collect();

    let last_ev = time_vec.last().unwrap();
    for (krate, metas) in krates {
        if let Some(last_pub) = metas.last() {
            if krate == "tar" { warn!("{:?}", last_pub.created_at) };
            if last_pub.created_at <= *last_ev {
                let idx = time_set.iter().position(|&t| t == last_pub.created_at).unwrap();
                let (before, time_to_pres) = time_vec.split_at(idx);

                if krate == "tar" {
                    info!("TIME LEN [{}] ({}) {}", time_vec.len(), before.len(), time_to_pres.len())
                };

                for ev in time_to_pres {
                    universe.extend_timeline(&krate, &ev);
                }
            }
        }
    }

    table
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

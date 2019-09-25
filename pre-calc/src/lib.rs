mod intern;

use cargo_tally::{Dependency, DependencyKind, Feature, DateTime, TranitiveDep};
use fnv::{FnvHashMap as Map, FnvHashSet as Set};
use indicatif::ProgressBar;
use log::{debug, info, warn, error};
use rayon::prelude::*;
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

    fn process_event(&mut self, event: Event) -> Vec<CrateKey> {
        debug!("processing event {} {}", event.name, event.num);

        let mut redo = Set::default();
        if let Some(prev) = self.crates.get(&event.name) {
            // events CrateName and index into Metadata in Universe.crates
            let prev_key = CrateKey::new(event.name, prev.len() as u32 - 1);

            for dep in &self.depends[&prev_key] {
                self.reverse_depends.get_mut(dep).unwrap().remove(&prev_key);
            }

            self.depends.remove(&prev_key);
            for (i, metadata) in prev.iter().enumerate() {
                // if event lies within 
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

        let mut to_update = Vec::new();

        // index is which dependency we mean in terms of Metadata
        let index = self.crates.entry(event.name).or_insert_with(Vec::new).len();
        let key = CrateKey::new(event.name, index as u32);

        let trans_res = self.resolve_and_add_to_graph(key, &metadata);
        to_update.extend(trans_res.crates.keys());

        self.reverse_depends.insert(key, Set::default());
        self.crates.get_mut(&event.name).unwrap().push(metadata);

        for outdated in redo.iter() {
            let metadata = self.crates[&outdated.name][outdated.index as usize].clone();
            warn!("re-resolving {} {}", outdated.name, metadata.num);
            let re_res = self.resolve_and_add_to_graph(*outdated, &metadata);
            to_update.extend(re_res.crates.keys());
        }
        to_update     
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

    fn resolve_and_add_to_graph(&mut self, key: CrateKey, metadata: &Metadata) -> Resolve {
        let mut d_resolve = Resolve { crates: Map::default(), };
        let mut t_resolve = Resolve { crates: Map::default(), };

        for dep in metadata.dependencies.iter() {
            // if the crate is in Universe.crates at the right version number
            if let Some(index) = self.resolve(&dep.name, &dep.req) {
                let name = crate_name(&dep.name);
                let key = CrateKey { name, index, };
                // direct deps just insert
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
        t_resolve
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
                    if let Some(deps) = self.reverse_depends.get(&key) {
                        set.extend(deps.iter().map(|key| key.name));
                        set.len()
                    } else {
                        0
                    }
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
        debug!("adding crate {} {}", key.name, metadata.num);
        // if not in Resolve.crates iter deps to find index of meta for each
        if !self.crates.contains_key(&key) {
            // array of indexs for universe.crates
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
    // for each version "event" this is the straight through vec
    let mut table = Vec::new();
    // for any changes that happen over time not at a version release event
    let mut extend = Vec::new();
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

        let redo = universe.process_event(ev);

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

        for redo_crate in redo.iter() {

            if redo_crate.name == "tar" { info!("CURRENT {} REDO {}", name, redo_crate.name) };

            let metas = &universe.crates[&redo_crate.name];
            let meta = metas[redo_crate.index as usize].clone();

            let row_update = universe.compute_counts(
                timestamp,
                redo_crate.name,
                meta.num.clone(),
                metas.to_vec(),
                redo_crate.index
            );

            let td = TranitiveDep {
                name: row_update.name.to_string(),
                timestamp,
                version: row_update.num.clone(),
                transitive_count: row_update.tran_counts,
                direct_count: row_update.dir_counts,
                total: row_update.total,
            };

            if let Some(last) = table.iter().find(|r| {
                r.name == td.name && r.version == td.version
            }) {
                info!(
                    "{:?} {:?} {:?} {} last {} {}",
                    timestamp,
                    td.direct_count,
                    td.transitive_count,
                    redo_crate.name,
                    last.direct_count,
                    last.transitive_count,
                );
            }
            
            // TransitiveDep compares all but the timestamp
            extend.push(td);
            //println!("{:?}", extend);
        }
        
        // // create points in time other than version releases 
        // let index = idx as u32;
        //     // for each version 
        // for (i, recent_meta) in universe.crates[&name].iter().enumerate() {

        //     // let key = CrateKey::new(*prev_krate, index);
        //     let row_update = universe.compute_counts(timestamp, name, recent_meta.num.clone(), universe.crates[&name].to_vec(), i as u32);
            
        //     if let Some(r) = table.iter().find(|r| {
        //         (r.transitive_count != row_update.tran_counts)
        //             || (r.direct_count != row_update.dir_counts)
        //             && (crate_name(&r.name) == row_update.name)
        //             && r.version == row_update.num
        //     }) {
        //         let td = TranitiveDep {
        //                 name: row_update.name.to_string(),
        //                 timestamp,
        //                 version: row_update.num.clone(),
        //                 transitive_count: row_update.tran_counts,
        //                 direct_count: row_update.dir_counts,
        //                 total: row_update.total,
        //             };
        //             // TransitiveDep compares all but the timestamp
        //             // if !extend.contains(&td) {
        //             //     // TODO this may not be needed when running on the whole crates.io index
        //             //     if td.transitive_count == 0 {
        //             //         td.transitive_count = r.transitive_count;
        //             //     }
        //             //     if td.direct_count == 0 {
        //             //         td.direct_count = r.direct_count;
        //             //     }
        //             //     println!("ADDING KRATE {:#?}", td);
                        
        //             // }
        //         extend.push(td);
        //     }
        // }
    }
    table.extend(extend);
    table
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}

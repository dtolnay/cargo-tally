use atty::{self, Stream::Stderr};
use cargo_tally::{Crate, DateTime, Dependency, DependencyKind, Feature};
use flate2::read::GzDecoder;
use fnv::{FnvHashMap as Map, FnvHashSet as Set};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use log::{debug, info};
use regex::Regex;
use semver::{Comparator, Op, Version, VersionReq};

use crate::csv::print_csv;
use crate::debug::CrateCollection;
use crate::error::{Error, Result};
use crate::graph::draw_graph;
use crate::intern::{crate_name, CrateName};
use crate::Args;

use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::u64;

#[derive(Debug)]
pub(crate) struct Universe {
    pub(crate) crates: Map<CrateName, Vec<Metadata>>,
    depends: Map<CrateKey, Vec<CrateKey>>,
    reverse_depends: Map<CrateKey, Set<CrateKey>>,
    transitive: bool,
}

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
struct Event {
    name: CrateName,
    num: Version,
    timestamp: DateTime,
    features: Map<String, Vec<Feature>>,
    dependencies: Vec<Dependency>,
}

#[derive(Debug)]
struct Matcher {
    name: CrateName,
    req: VersionReq,
    nodes: Vec<u32>,
}

#[derive(Debug)]
pub(crate) struct Row {
    pub(crate) timestamp: DateTime,
    pub(crate) name: CrateName,
    pub(crate) num: Version,
    pub(crate) counts: Vec<usize>,
    pub(crate) total: usize,
}

impl CrateKey {
    fn new(name: CrateName, index: u32) -> Self {
        CrateKey { name, index }
    }
}

impl Universe {
    fn new(transitive: bool) -> Self {
        Universe {
            crates: Map::default(),
            depends: Map::default(),
            reverse_depends: Map::default(),
            transitive,
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

    fn process_event(&mut self, event: Event) {
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

    fn resolve_and_add_to_graph(&mut self, key: CrateKey, metadata: &Metadata) {
        let mut resolve = Resolve {
            crates: Map::default(),
        };

        for dep in &metadata.dependencies {
            if let Some(index) = self.resolve(&dep.name, &dep.req) {
                let key = CrateKey::new(crate_name(&*dep.name), index);
                if self.transitive {
                    resolve.add_crate(self, key, dep.default_features, &dep.features);
                } else {
                    resolve.crates.insert(key, ResolvedCrate::no_resolve());
                }
            }
        }

        debug!(
            "depends on {:?}",
            CrateCollection::new(self, resolve.crates.keys())
        );

        for dep in resolve.crates.keys() {
            self.reverse_depends
                .entry(*dep)
                .or_insert_with(Set::default)
                .insert(key);
        }
        self.depends
            .insert(key, resolve.crates.keys().copied().collect());
    }

    fn compute_counts(
        &self,
        timestamp: DateTime,
        name: CrateName,
        num: Version,
        matchers: &[Matcher],
    ) -> Row {
        let mut set = Set::default();
        Row {
            timestamp,
            name,
            num,
            counts: matchers
                .iter()
                .map(|matcher| {
                    set.clear();
                    for index in &matcher.nodes {
                        let key = CrateKey::new(matcher.name, *index);
                        set.extend(self.reverse_depends[&key].iter().map(|key| key.name));
                    }
                    set.len()
                })
                .collect(),
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

pub(crate) fn tally(args: &Args) -> Result<()> {
    let mut chronology = load_data(args)?;
    chronology.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let mut universe = Universe::new(args.transitive);
    let mut matchers = create_matchers(args)?;
    let mut table = Vec::<Row>::new();

    let n = chronology.len() as u64;
    let pb = ProgressBar::hidden();
    if atty::is(Stderr) {
        pb.set_length(n * n);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("[{wide_bar:.cyan/blue}] {percent}%")
                .progress_chars("&&."),
        );
        pb.set_draw_target(ProgressDrawTarget::stderr());
    }
    for (i, event) in chronology.into_iter().enumerate() {
        let name = event.name;
        let num = event.num.clone();
        let timestamp = event.timestamp;
        universe.process_event(event);
        for matcher in &mut matchers {
            if matcher.name == name && matcher.req.matches(&num) {
                matcher.nodes.push(universe.crates[&name].len() as u32 - 1);
            }
        }
        let row = universe.compute_counts(timestamp, name, num, &matchers);
        let include = match table.last() {
            None => row.counts.iter().any(|&count| count != 0),
            Some(last) => last.counts != row.counts,
        };
        if include {
            table.push(row);
        }
        pb.inc(2 * i as u64 + 1);
    }
    pb.finish_and_clear();
    if table.is_empty() {
        return Err(Error::NothingFound);
    }

    if args.title.is_some() {
        draw_graph(args, &table);
    } else {
        print_csv(args, &table);
    }

    Ok(())
}

fn load_data(args: &Args) -> Result<Vec<Event>> {
    let mut chronology = Vec::new();

    let exclude = match args.exclude {
        Some(ref exclude) => Some(Regex::new(exclude)?),
        None => None,
    };

    let json_path = Path::new(cargo_tally::JSONFILE);
    if !json_path.exists() {
        return Err(Error::MissingJson);
    }

    let file = File::open(json_path)?;
    let mut decoder = GzDecoder::new(file);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    let de = serde_json::Deserializer::from_slice(&decompressed);

    for line in de.into_iter::<Crate>() {
        let krate = line?;

        if let Some(exclude) = &exclude {
            if exclude.is_match(&krate.name) {
                continue;
            }
        }

        chronology.push(Event {
            name: crate_name(&*krate.name),
            num: krate.version,
            timestamp: krate.published.unwrap(),
            features: krate.features,
            dependencies: krate.dependencies,
        });
    }

    Ok(chronology)
}

fn create_matchers(args: &Args) -> Result<Vec<Matcher>> {
    let mut matchers = Vec::new();

    for s in &args.crates {
        let mut pieces = s.splitn(2, ':');
        matchers.push(Matcher {
            name: crate_name(pieces.next().unwrap()),
            req: match pieces.next().unwrap_or("*").parse() {
                Ok(req) => req,
                Err(err) => return Err(Error::ParseSeries(s.clone(), err)),
            },
            nodes: Vec::new(),
        });
    }

    Ok(matchers)
}

fn compatible_req(version: &Version) -> VersionReq {
    VersionReq {
        comparators: vec![Comparator {
            op: Op::Caret,
            major: version.major,
            minor: Some(version.minor),
            patch: Some(version.patch),
            pre: version.pre.clone(),
        }],
    }
}

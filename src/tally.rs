use cargo_tally::{DateTime, Dependency, DependencyKind, Feature};
use cargo_tally::{cache_crate, cache_dependencies, cache_index, num_pages};
use csv::print_csv;
use debug::CrateCollection;
use failure::{self, Error};
use fnv::{FnvHashMap as Map, FnvHashSet as Set};
use graph::draw_graph;
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use intern::{crate_name, CrateName};
use isatty::stderr_isatty;
use regex::Regex;
use semver::{Version, VersionReq};
use semver_parser::range::Op::Compatible;
use semver_parser::range::{self, Predicate};

use Flags;

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
                if is_compatible(&metadata.num, &event.num) {
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

        let metadata = Metadata {
            num: event.num,
            created_at: event.timestamp,
            features: event.features,
            dependencies: event.dependencies,
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

        trace!(
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
            .insert(key, resolve.crates.keys().cloned().collect());
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
            key.name,
            metadata.num,
            feature
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
                panic!(
                    "feature not found: {} {}:{}/{}",
                    key.name,
                    metadata.num,
                    name,
                    feature
                );
            }
        }
    }

    fn add_crate_feature(&mut self, universe: &Universe, key: CrateKey, feature: &str) {
        let metadata = &universe.crates[&key.name][key.index as usize];

        if !self.crates
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

pub(crate) fn tally(flags: &Flags) -> Result<(), Error> {
    let mut chronology = load_data(flags)?;
    chronology.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let mut universe = Universe::new(flags.flag_transitive);
    let mut matchers = create_matchers(flags)?;
    let mut table = Vec::<Row>::new();

    let n = chronology.len() as u64;
    let pb = ProgressBar::hidden();
    if stderr_isatty() {
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
        return Err(failure::err_msg("nothing found for this crate"));
    }

    if flags.flag_graph.is_some() {
        draw_graph(flags, &table);
    } else {
        print_csv(flags, &table);
    }

    Ok(())
}

fn load_data(flags: &Flags) -> Result<Vec<Event>, Error> {
    let mut chronology = Vec::new();

    let exclude = match flags.flag_exclude {
        Some(ref exclude) => Some(Regex::new(exclude)?),
        None => None,
    };

    for p in 1..num_pages()? + 1 {
        for krate in cache_index(p)?.crates {
            if let Some(ref exclude) = exclude {
                if exclude.is_match(&krate.name) {
                    continue;
                }
            }
            let krate = cache_crate(&krate.name)?;
            for version in krate.versions {
                chronology.push(Event {
                    name: crate_name(&*krate.index.name),
                    num: version.num.clone(),
                    timestamp: version.created_at,
                    features: version.features,
                    dependencies: cache_dependencies(&krate.index.name, &version.num)?.dependencies,
                });
            }
        }
    }

    Ok(chronology)
}

fn create_matchers(flags: &Flags) -> Result<Vec<Matcher>, Error> {
    let mut matchers = Vec::new();

    for s in &flags.arg_crate {
        let mut pieces = s.splitn(2, ':');
        matchers.push(Matcher {
            name: crate_name(pieces.next().unwrap()),
            req: match pieces.next().unwrap_or("*").parse() {
                Ok(req) => req,
                Err(err) => {
                    return Err(failure::err_msg(
                        format!("Failed to parse series {}: {}", s, err),
                    ));
                }
            },
            nodes: Vec::new(),
        });
    }

    Ok(matchers)
}

fn is_compatible(older: &Version, newer: &Version) -> bool {
    use semver::Identifier as SemverId;
    use semver_parser::version::Identifier as ParseId;
    let req = range::VersionReq {
        predicates: vec![
            Predicate {
                op: Compatible,
                major: older.major,
                minor: Some(older.minor),
                patch: Some(older.patch),
                pre: older
                    .pre
                    .iter()
                    .map(|pre| match *pre {
                        SemverId::Numeric(n) => ParseId::Numeric(n),
                        SemverId::AlphaNumeric(ref s) => ParseId::AlphaNumeric(s.clone()),
                    })
                    .collect(),
            },
        ],
    };
    VersionReq::from(req).matches(newer)
}

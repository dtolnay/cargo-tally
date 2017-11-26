#![cfg_attr(feature = "cargo-clippy", allow(
    let_and_return,
    map_entry,
    needless_pass_by_value,
    redundant_closure_call,
))]

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

extern crate cargo;
extern crate chrono;
extern crate env_logger;
extern crate failure;
extern crate flate2;
extern crate fnv;
extern crate gnuplot;
extern crate indicatif;
extern crate isatty;
extern crate palette;
extern crate regex;
extern crate reqwest;
extern crate semver;
extern crate semver_parser;
extern crate serde;
extern crate string_interner;
extern crate tar;
extern crate unindent;

use cargo::{CliResult, CliError};
use cargo::core::shell::Shell;
use cargo::util::{Config, CargoError};

use chrono::{Utc, NaiveDate, NaiveTime, NaiveDateTime};

use failure::Error;

use flate2::read::GzDecoder;

use fnv::{FnvHashSet as Set, FnvHashMap as Map};

use gnuplot::{Figure, Fix, Auto, Caption, LineWidth, AxesCommon, Color,
              MinorScale, Graph, Placement, AlignLeft, AlignTop};

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use isatty::stderr_isatty;

use palette::Hue;
use palette::pixel::Srgb;

use regex::Regex;

use reqwest::header::ContentLength;

use semver::{Version, VersionReq};
use semver_parser::range::{self, Predicate};
use semver_parser::range::Op::Compatible;

use tar::Archive;

use unindent::unindent;

use std::env;
use std::fmt::{self, Debug};
use std::path::Path;
use std::u64;

extern crate cargo_tally;
use cargo_tally::*;

mod progress;
use progress::ProgressRead;

mod intern;
use intern::*;

#[cfg_attr(rustfmt, rustfmt_skip)]
const USAGE: &str = "
Tally the number of crates that depend on a group of crates over time.

Usage: cargo tally --init
       cargo tally [options] <crate>...
       cargo tally (--help | --version)

Options:
    -h, --help        Print this message
    -V, --version     Print version info and exit
    --graph TITLE     Display line graph using gnuplot, rather than dump csv
    --relative        Display as a fraction of total crates, not absolute number
    --transitive      Count transitive dependencies, not just direct dependencies
    --exclude REGEX   Ignore a dependency coming from any crates matching regex
";

#[derive(Deserialize, Debug)]
struct Flags {
    flag_init: bool,
    arg_crate: Vec<String>,
    flag_version: bool,
    flag_graph: Option<String>,
    flag_relative: bool,
    flag_transitive: bool,
    flag_exclude: Option<String>,
}

fn main() {
    let config = match Config::default() {
        Ok(cfg) => cfg,
        Err(e) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(e.into(), &mut shell)
        }
    };

    let result = (|| {
        let args: Vec<_> = try!(
            env::args_os()
                .map(|s| {
                    s.into_string().map_err(|s| {
                        CargoError::from(format!("invalid unicode in argument: {:?}", s))
                    })
                })
                .collect());
        let rest = &args;
        cargo::call_main_without_stdin(real_main, &config, USAGE, rest, false)
    })();

    match result {
        Err(e) => cargo::exit_with_error(e, &mut *config.shell()),
        Ok(()) => {}
    }
}

fn real_main(flags: Flags, _config: &Config) -> CliResult {
    if flags.flag_version {
        println!("cargo-tally {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let result = if flags.flag_init {
        init()
    } else {
        tally(&flags)
    };

    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            eprintln!("{}", err);
            Err(CliError::code(1))
        }
    }
}

fn init() -> Result<(), Error> {
    let tally_path = Path::new("tally");
    if tally_path.exists() {
        let pwd = env::current_dir().unwrap_or_else(|_| Path::new(".").to_owned());
        let helpful_path = pwd.join(tally_path);

        return Err(failure::err_msg(unindent(&format!("
            Already exists: {}
            Remove and run `cargo tally --init` again.\
        ", helpful_path.display()))));
    }

    let snapshot = "https://github.com/dtolnay/cargo-tally/releases/download/2017-11-19/tally.tgz";
    let tgz = reqwest::get(snapshot)?.error_for_status()?;

    let pb = ProgressBar::hidden();
    if stderr_isatty() {
        if let Some(&ContentLength(n)) = tgz.headers().get() {
            pb.set_length(n);
            pb.set_style(ProgressStyle::default_bar()
                .template("[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
                .progress_chars("&&."));
            pb.set_draw_target(ProgressDrawTarget::stderr());
        }
    }

    let tracker = ProgressRead::new(&pb, tgz);
    let decoder = GzDecoder::new(tracker)?;
    let mut archive = Archive::new(decoder);
    archive.unpack(".")?;

    pb.finish_and_clear();
    Ok(())
}

#[derive(Debug)]
struct Universe {
    crates: Map<CrateName, Vec<Metadata>>,
    depends: Map<CrateKey, Vec<CrateKey>>,
    reverse_depends: Map<CrateKey, Set<CrateKey>>,
    transitive: bool,
}

#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct CrateKey {
    name: CrateName,
    index: u32,
}

#[derive(Clone, Debug)]
struct Metadata {
    num: Version,
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
struct Row {
    timestamp: DateTime,
    name: CrateName,
    num: Version,
    counts: Vec<usize>,
    total: usize,
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
            .get(&intern(name))?
            .iter()
            .enumerate()
            .rev()
            .find(|&(_, metadata)| req.matches(&metadata.num))
            .map(|(i, _)| i as u32)
    }

    fn process_event(&mut self, event: Event, matchers: &[Matcher]) {
        info!("processing event {} {}", event.name, event.num);

        let mut redo = Set::default();
        if let Some(prev) = self.crates.get(&event.name) {
            {
                let key = CrateKey { name: event.name, index: prev.len() as u32 - 1 };
                for dep in &self.depends[&key] {
                    self.reverse_depends.get_mut(dep).unwrap().remove(&key);
                }
                self.depends.remove(&key);
            }
            for (i, metadata) in prev.iter().enumerate() {
                if is_compatible(&metadata.num, &event.num) {
                    let key = CrateKey { name: event.name, index: i as u32 };
                    for node in self.reverse_depends[&key].clone() {
                        let mut upstream_of_matcher = false;
                        'matchers: for matcher in matchers {
                            for index in &matcher.nodes {
                                let key = CrateKey { name: matcher.name, index: *index as u32 };
                                if self.reverse_depends[&key].contains(&node) {
                                    upstream_of_matcher = true;
                                    break 'matchers;
                                }
                            }
                        }
                        if upstream_of_matcher {
                            for dep in &self.depends[&node] {
                                self.reverse_depends.get_mut(dep).unwrap().remove(&node);
                            }
                            redo.insert(node);
                        }
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
        let key = CrateKey { name: event.name, index: index as u32 };
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
                let key = CrateKey { name: intern(&*dep.name), index: index as u32 };
                if self.transitive {
                    resolve.add_crate(self, key, dep.default_features, &dep.features);
                } else {
                    resolve.crates.insert(key, ResolvedCrate::no_resolve());
                }
            }
        }

        trace!("depends on {:?}", CrateCollection { universe: self, crates: resolve.crates.keys() });

        for dep in resolve.crates.keys() {
            self.reverse_depends.entry(*dep).or_insert_with(Set::default).insert(key);
        }
        self.depends.insert(key, resolve.crates.keys().cloned().collect());
    }

    fn compute_counts(&self, timestamp: DateTime, name: CrateName, num: Version, matchers: &[Matcher]) -> Row {
        let mut set = Set::default();
        Row {
            timestamp,
            name,
            num,
            counts: matchers.iter()
                .map(|matcher| {
                    set.clear();
                    for index in &matcher.nodes {
                        let key = CrateKey { name: matcher.name, index: *index as u32 };
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

struct CrateCollection<'a, I> {
    universe: &'a Universe,
    crates: I,
}

impl<'a, I> Debug for CrateCollection<'a, I>
    where I: Clone + IntoIterator<Item = &'a CrateKey>
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        struct DebugCrate<'a> {
            universe: &'a Universe,
            key: CrateKey,
        }

        impl<'a> Debug for DebugCrate<'a> {
            fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                let name = self.key.name;
                let num = &self.universe.crates[&name][self.key.index as usize].num;
                write!(formatter, "{}:{}", name, num)
            }
        }

        let crates = self.crates.clone().into_iter().map(|&key| DebugCrate { universe: self.universe, key });
        formatter.debug_list().entries(crates).finish()
    }
}

impl Resolve {
    fn add_crate(&mut self, universe: &Universe, key: CrateKey, default_features: bool, features: &[String]) {
        let metadata = &universe.crates[&key.name][key.index as usize];

        debug!("adding crate {} {}", key.name, metadata.num);

        if !self.crates.contains_key(&key) {
            let resolved = metadata.dependencies.iter()
                .map(|dep| universe.resolve(&dep.name, &dep.req))
                .collect::<Vec<_>>();
            self.crates.insert(key, ResolvedCrate {
                features: Set::default(),
                resolved: resolved.clone(),
            });
            for (dep, index) in metadata.dependencies.iter().zip(resolved) {
                if !dep.optional && dep.kind != DependencyKind::Dev && index.is_some() {
                    let key = CrateKey { name: intern(&*dep.name), index: index.unwrap() };
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

        debug!("adding dep or feature {} {}:{}", key.name, metadata.num, feature);

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
                            let key = CrateKey { name: intern(&**name), index: resolved };
                            self.add_crate(universe, key, dep.default_features, &dep.features);
                            self.add_crate_feature(universe, key, feature);
                        }
                        return;
                    }
                }
                panic!("feature not found: {} {}:{}/{}", key.name, metadata.num, name, feature);
            }
        }
    }

    fn add_crate_feature(&mut self, universe: &Universe, key: CrateKey, feature: &str) {
        let metadata = &universe.crates[&key.name][key.index as usize];

        if !self.crates.get_mut(&key).unwrap().features.insert(feature.to_owned()) {
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
                        let key = CrateKey { name: intern(feature), index: resolved };
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

fn tally(flags: &Flags) -> Result<(), Error> {
    let mut chronology = load_data(flags)?;
    chronology.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let mut universe = Universe::new(flags.flag_transitive);
    let mut matchers = create_matchers(flags)?;
    let mut table = Vec::<Row>::new();

    let n = chronology.len() as u64;
    let pb = ProgressBar::hidden();
    if stderr_isatty() {
        pb.set_length(n * n);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{wide_bar:.cyan/blue}] {percent}%")
            .progress_chars("&&."));
        pb.set_draw_target(ProgressDrawTarget::stderr());
    }
    for (i, event) in chronology.into_iter().enumerate() {
        let name = event.name;
        let num = event.num.clone();
        let timestamp = event.timestamp;
        universe.process_event(event, &matchers);
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
                    name: intern(&*krate.index.name),
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
            name: intern(pieces.next().unwrap()),
            req: match pieces.next().unwrap_or("*").parse() {
                Ok(req) => req,
                Err(err) => {
                    return Err(failure::err_msg(format!(
                        "Failed to parse series {}: {}", s, err)));
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
                pre: older.pre.iter().map(|pre| {
                    match *pre {
                        SemverId::Numeric(n) => ParseId::Numeric(n),
                        SemverId::AlphaNumeric(ref s) => ParseId::AlphaNumeric(s.clone()),
                    }
                }).collect(),
            },
        ],
    };
    VersionReq::from(req).matches(newer)
}

fn draw_graph(flags: &Flags, table: &[Row]) {
    let mut colors = Vec::new();
    let mut captions = Vec::new();
    let primary: palette::Color = Srgb::new_u8(200, 80, 40).into();
    let n = flags.arg_crate.len();
    for i in 0..n {
        let linear = primary.shift_hue((360.0 * (i as f32) / (n as f32)).into());
        let srgb = Srgb::from_linear(linear);
        let red = (srgb.red * 256.0) as u8;
        let green = (srgb.green * 256.0) as u8;
        let blue = (srgb.blue * 256.0) as u8;
        let hex = format!("#{:02X}{:02X}{:02X}", red, green, blue);
        colors.push(hex);
        captions.push(flags.arg_crate[i].replace('_', "\\\\_"));
    }

    let mut fg = Figure::new();
    {
        // Create plot
        let axes = fg.axes2d();
        axes.set_title(flags.flag_graph.as_ref().unwrap(), &[]);
        axes.set_x_range(
            Fix(float_year(&table[0].timestamp) - 0.3),
            Fix(float_year(&Utc::now()) + 0.15));
        axes.set_y_range(Fix(0.0), Auto);
        axes.set_x_ticks(Some((Fix(1.0), 12)), &[MinorScale(2.0)], &[]);
        axes.set_legend(Graph(0.05), Graph(0.9), &[Placement(AlignLeft, AlignTop)], &[]);

        // Create x-axis
        let mut x = Vec::new();
        for row in table {
            x.push(float_year(&row.timestamp));
        }

        // Create series
        for i in 0..n {
            if flags.flag_relative {
                let mut y = Vec::new();
                for row in table {
                    y.push(row.counts[i] as f32 / row.total as f32);
                }
                axes.lines(&x, &y, &[Caption(&captions[i]), LineWidth(1.5), Color(&colors[i])]);
            } else {
                let mut y = Vec::new();
                for row in table {
                    y.push(row.counts[i]);
                }
                axes.lines(&x, &y, &[Caption(&captions[i]), LineWidth(1.5), Color(&colors[i])]);
            }
        }
    }
    fg.show();
}

fn float_year(dt: &DateTime) -> f64 {
    let nd = NaiveDate::from_ymd(2017, 1, 1);
    let nt = NaiveTime::from_hms_milli(0, 0, 0, 0);
    let base = DateTime::from_utc(NaiveDateTime::new(nd, nt), Utc);
    let offset = dt.signed_duration_since(base);
    let year = offset.num_minutes() as f64 / 525960.0 + 2017.0;
    year
}

fn print_csv(flags: &Flags, table: &[Row]) {
    print!("timestamp");
    for s in &flags.arg_crate {
        print!(",{}", s);
    }
    println!();

    let detail = env::var("DETAIL").is_ok();

    for row in table {
        print!("{}", row.timestamp.format("%m/%d/%Y %H:%M"));
        if detail {
            print!(",{}:{}", row.name, row.num);
        }
        for &column in &row.counts {
            if flags.flag_relative {
                print!(",{}", column as f32 / row.total as f32);
            } else {
                print!(",{}", column);
            }
        }
        println!();
    }
}

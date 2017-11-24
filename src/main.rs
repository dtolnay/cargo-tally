#[macro_use]
extern crate serde_derive;

extern crate cargo;
extern crate chrono;
extern crate failure;
extern crate flate2;
extern crate fnv;
extern crate gnuplot;
extern crate indicatif;
extern crate isatty;
extern crate palette;
extern crate petgraph;
extern crate regex;
extern crate reqwest;
extern crate semver;
extern crate semver_parser;
extern crate serde;
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

use petgraph::{Incoming, Outgoing};
use petgraph::graph::NodeIndex;

use regex::Regex;

use reqwest::header::ContentLength;

use semver::{Version, VersionReq};
use semver_parser::range::{self, Predicate};
use semver_parser::range::Op::Compatible;

use tar::Archive;

use unindent::unindent;

use std::env;
use std::fmt::{self, Display};
use std::mem;
use std::path::Path;
use std::u64;

extern crate cargo_tally;
use cargo_tally::*;

mod progress;
use progress::ProgressRead;

#[cfg_attr(rustfmt, rustfmt_skip)]
const USAGE: &'static str = "
Tally the number of crates that depend on a group of crates over time.

Usage: cargo tally --init
       cargo tally [options] <crate>...
       cargo tally (--help | --version)

Options:
    -h, --help        Print this message
    -V, --version     Print version info and exit
    --graph TITLE     Display line graph using gnuplot, rather than dump csv
    --relative        Display as a fraction of total crates, not absolute number
    --transitive      (not implemented) Count transitive dependencies, not
                      just direct dependencies
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
    let mut config = match Config::default() {
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
        cargo::call_main_without_stdin(real_main, &mut config, USAGE, rest, false)
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
        tally(flags)
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
        let pwd = env::current_dir().unwrap_or(Path::new(".").to_owned());
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
    graph: petgraph::Graph<Key, Edge>,
    crates: Map<String, Vec<NodeIndex>>,
}

#[derive(Debug)]
struct Key {
    name: String,
    num: Version,
    crates_incoming: Set<String>,
}

impl Display for Key {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}:{}", self.name, self.num)
    }
}

#[derive(Debug)]
enum Edge {
    Current {
        kind: DependencyKind,
        req: VersionReq,
        optional: bool,
        default_features: bool,
        features: Vec<String>,
    },
    Obsolete,
}

impl Display for Edge {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Edge::Current { kind, .. } => write!(formatter, "{}", kind),
            Edge::Obsolete => write!(formatter, "obsolete"),
        }
    }
}

#[derive(Debug)]
struct Event {
    name: String,
    num: Version,
    timestamp: DateTime,
    dependencies: Vec<Dependency>,
}

#[derive(Debug)]
struct Matcher {
    name: String,
    req: VersionReq,
    matches: Vec<NodeIndex>,
}

#[derive(Debug)]
struct Row {
    timestamp: DateTime,
    counts: Vec<usize>,
    total: usize,
}

impl Universe {
    fn new() -> Self {
        Universe {
            graph: petgraph::Graph::new(),
            crates: Map::default(),
        }
    }
}

fn tally(flags: Flags) -> Result<(), Error> {
    if flags.flag_transitive {
        return Err(failure::err_msg("--transitive is not implemented"));
    }

    let mut chronology = load_data(&flags)?;
    chronology.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let mut universe = Universe::new();
    let mut matchers = create_matchers(&flags)?;
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
        let timestamp = event.timestamp.clone();
        process_event(&mut universe, &mut matchers, event);
        let row = compute_counts(&universe, &matchers, timestamp);
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
        draw_graph(&flags, table);
    } else {
        print_csv(&flags, table);
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
                    name: krate.index.name.clone(),
                    num: version.num.clone(),
                    timestamp: version.created_at,
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
            name: pieces.next().unwrap().to_owned(),
            req: match pieces.next().unwrap_or("*").parse() {
                Ok(req) => req,
                Err(err) => {
                    return Err(failure::err_msg(format!(
                        "Failed to parse series {}: {}", s, err)));
                }
            },
            matches: Vec::new(),
        });
    }

    Ok(matchers)
}

fn process_event(universe: &mut Universe, matchers: &mut [Matcher], event: Event) {
    // Insert new node in graph
    let key = Key {
        name: event.name.clone(),
        num: event.num.clone(),
        crates_incoming: Set::default(),
    };
    let new = universe.graph.add_node(key);

    // If there is an older version of this crate, remove its name from
    // everything it depends on
    if let Some(older) = universe.crates.get(&event.name) {
        if let Some(&last) = older.last() {
            let mut walk = universe.graph.neighbors_directed(last, Outgoing).detach();
            while let Some(edge) = walk.next_edge(&universe.graph) {
                let endpoints = universe.graph.edge_endpoints(edge).unwrap();
                universe.graph[endpoints.1].crates_incoming.remove(&event.name);
                mem::replace(&mut universe.graph[edge], Edge::Obsolete);
            }
        }
    }

    // Add edges to all nodes depended on by the new node
    for dep in event.dependencies {
        if dep.name != event.name {
            if let Some(target) = resolve(universe, &dep.name, &dep.req) {
                universe.graph.add_edge(new, target, Edge::Current {
                    kind: dep.kind,
                    req: dep.req,
                    optional: dep.optional,
                    default_features: dep.default_features,
                    features: dep.features,
                });
                universe.graph[target].crates_incoming.insert(event.name.clone());
            }
        }
    }

    // Find all nodes representing older versions of the same crate
    let older = universe.crates.entry(event.name.clone()).or_insert_with(Vec::new);

    // Update edges that previously depended on older version of this crate
    for &node in &*older {
        if is_compatible(&universe.graph[node].num, &event.num) {
            let mut walk = universe.graph.neighbors_directed(node, Incoming).detach();
            while let Some(edge) = walk.next_edge(&universe.graph) {
                let mut repoint = false;
                if let Edge::Current { ref req, .. } = universe.graph[edge] {
                    repoint = req.matches(&event.num);
                }
                if repoint {
                    let endpoints = universe.graph.edge_endpoints(edge).unwrap();
                    let old = mem::replace(&mut universe.graph[edge], Edge::Obsolete);
                    universe.graph.add_edge(endpoints.0, new, old);
                    let reverse_dep = universe.graph[endpoints.0].name.clone();
                    if universe.graph[endpoints.1].crates_incoming.remove(&reverse_dep) {
                        universe.graph[new].crates_incoming.insert(reverse_dep);
                    }
                }
            }
        }
    }

    // Update matchers that tally the new node
    for matcher in matchers {
        if matcher.name == event.name && matcher.req.matches(&event.num) {
            matcher.matches.push(new);
        }
    }

    // Add new node to list of versions of its crate
    older.push(new);
}

fn resolve(universe: &Universe, name: &str, req: &VersionReq) -> Option<NodeIndex> {
    let versions = universe.crates.get(name)?;
    let mut max = None::<NodeIndex>;
    for &node in versions {
        let key = &universe.graph[node];
        if req.matches(&key.num) {
            if max.map(|max| key.num > universe.graph[max].num).unwrap_or(true) {
                max = Some(node);
            }
        }
    }
    Some(max.unwrap_or(*versions.last().unwrap()))
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

fn compute_counts(universe: &Universe, matchers: &[Matcher], timestamp: DateTime) -> Row {
    let mut crates = Set::default();
    Row {
        timestamp: timestamp,
        counts: matchers.iter()
            .map(|matcher| {
                crates.clear();
                for &node in &matcher.matches {
                    crates.extend(&universe.graph[node].crates_incoming);
                }
                crates.len()
            })
            .collect(),
        total: universe.crates.len(),
    }
}

fn draw_graph(flags: &Flags, table: Vec<Row>) {
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
        for row in &table {
            x.push(float_year(&row.timestamp));
        }

        // Create series
        for i in 0..n {
            if flags.flag_relative {
                let mut y = Vec::new();
                for row in &table {
                    y.push(row.counts[i] as f32 / row.total as f32);
                }
                axes.lines(&x, &y, &[Caption(&captions[i]), LineWidth(1.5), Color(&colors[i])]);
            } else {
                let mut y = Vec::new();
                for row in &table {
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
    let offset = dt.signed_duration_since(base.clone());
    let year = offset.num_minutes() as f64 / 525960.0 + 2017.0;
    year
}

fn print_csv(flags: &Flags, table: Vec<Row>) {
    print!("timestamp");
    for s in &flags.arg_crate {
        print!(",{}", s);
    }
    println!();

    for row in table {
        print!("{}", row.timestamp.format("%m/%d/%Y %H:%M"));
        for column in row.counts {
            if flags.flag_relative {
                print!(",{}", column as f32 / row.total as f32);
            } else {
                print!(",{}", column);
            }
        }
        println!();
    }
}

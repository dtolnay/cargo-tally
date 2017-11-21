#[macro_use]
extern crate serde_derive;

extern crate cargo;
extern crate chrono;
extern crate failure;
extern crate flate2;
extern crate gnuplot;
extern crate indicatif;
extern crate palette;
extern crate regex;
extern crate reqwest;
extern crate semver;
extern crate serde;
extern crate tar;
extern crate unindent;

use cargo::{CliResult, CliError};
use cargo::core::shell::Shell;
use cargo::util::{Config, CargoError};

use chrono::{DateTime, Utc, NaiveDate, NaiveTime, NaiveDateTime};

use failure::Error;

use flate2::read::GzDecoder;

use gnuplot::{Figure, Fix, Auto, Caption, LineWidth, AxesCommon, Color,
              MinorScale, Graph, Placement, AlignLeft, AlignTop};

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use palette::Hue;
use palette::pixel::Srgb;

use regex::Regex;

use reqwest::header::ContentLength;

use semver::Version;

use tar::Archive;

use unindent::unindent;

use std::collections::HashSet as Set;
use std::env;
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
    if let Some(&ContentLength(n)) = tgz.headers().get() {
        pb.set_length(n);
        pb.set_style(ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));
        pb.set_draw_target(ProgressDrawTarget::stderr());
    }

    let tracker = ProgressRead::new(&pb, tgz);
    let decoder = GzDecoder::new(tracker)?;
    let mut archive = Archive::new(decoder);
    archive.unpack(".")?;

    pb.finish_with_message("ready to tally!");
    Ok(())
}

#[derive(Debug)]
struct Event {
    name: String,
    num: Version,
    timestamp: DateTime<Utc>,
    dependencies: Vec<Dependency>,
}

#[derive(Debug)]
struct Matcher {
    name: String,
    num: Option<Version>,
    crates_using: Set<String>,
}

#[derive(Debug)]
struct Row {
    timestamp: DateTime<Utc>,
    counts: Vec<usize>,
    total: usize,
}

fn tally(flags: Flags) -> Result<(), Error> {
    if flags.flag_transitive {
        return Err(failure::err_msg("--transitive is not implemented"));
    }

    let mut chronology = load_data(&flags)?;
    chronology.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

    let mut matchers = create_matchers(&flags)?;

    let mut table = Vec::new();
    let mut all_crates = Set::new();
    for event in chronology {
        all_crates.insert(event.name.clone());
        let changed = process_event(&mut matchers, &event)?;
        if changed {
            table.push(Row {
                timestamp: event.timestamp,
                counts: matchers.iter().map(|m| m.crates_using.len()).collect(),
                total: all_crates.len(),
            });
        }
    }
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
            num: match pieces.next() {
                Some(num) => {
                    match parse_major_minor(num) {
                        Ok(num) => Some(num),
                        Err(_) => {
                            return Err(failure::err_msg(format!(
                                "Failed to parse series {:?}, \
                                 expected something like \"serde:0.9\"", s)));
                        }
                    }
                }
                None => None,
            },
            crates_using: Set::new(),
        });
    }

    Ok(matchers)
}

fn parse_major_minor(num: &str) -> Result<Version, Error> {
    let mut pieces = num.splitn(2, '.');
    let major = pieces.next().unwrap().parse()?;
    let minor = pieces.next().ok_or(failure::err_msg("missing minor"))?.parse()?;
    Ok(Version::new(major, minor, u64::MAX))
}

fn process_event(matchers: &mut [Matcher], event: &Event) -> Result<bool, Error> {
    let mut changed = false;

    for matcher in matchers {
        let mut using = false;
        for dep in &event.dependencies {
            if dep.name == matcher.name {
                using = true;
                let matches = match matcher.num {
                    Some(ref version) => {
                        // Exclude silly wildcard deps
                        if dep.req.matches(&Version::new(0, u64::MAX, 0)) {
                            false
                        } else if dep.req.matches(&Version::new(u64::MAX, 0, 0)) {
                            false
                        } else {
                            dep.req.matches(version)
                        }
                    }
                    None => true,
                };
                changed |= if matches {
                    matcher.crates_using.insert(event.name.clone())
                } else {
                    matcher.crates_using.remove(&event.name)
                };
            }
        }
        if !using {
            changed |= matcher.crates_using.remove(&event.name);
        }
    }

    Ok(changed)
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

fn float_year(dt: &DateTime<Utc>) -> f64 {
    let nd = NaiveDate::from_ymd(2017, 1, 1);
    let nt = NaiveTime::from_hms_milli(0, 0, 0, 0);
    let base = DateTime::<Utc>::from_utc(NaiveDateTime::new(nd, nt), Utc);
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

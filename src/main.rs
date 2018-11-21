#![allow(
    clippy::let_and_return,
    clippy::map_entry,
    clippy::needless_pass_by_value,
    clippy::redundant_closure_call,
    clippy::unreadable_literal,
)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[allow(unused_imports)]
#[macro_use]
extern crate structopt;

extern crate atty;
extern crate cargo;
extern crate cargo_tally;
extern crate chrono;
extern crate env_logger;
extern crate failure;
extern crate flate2;
extern crate fnv;
extern crate gnuplot;
extern crate indicatif;
extern crate palette;
extern crate regex;
extern crate reqwest;
extern crate semver;
extern crate semver_parser;
extern crate serde;
extern crate string_interner;
extern crate tar;
extern crate unindent;

use cargo::core::shell::Shell;
use cargo::{CliError, CliResult, Config};
use structopt::clap::AppSettings;
use structopt::StructOpt;

use std::env;
use std::io::Write;
use std::process;

mod csv;
mod debug;
mod graph;
mod init;
mod intern;
mod progress;
mod tally;

use init::init;
use tally::tally;

#[derive(StructOpt)]
#[structopt(bin_name = "cargo", author = "1")]
enum Opts {
    #[structopt(
        name = "tally",
        raw(
            setting = "AppSettings::UnifiedHelpMessage",
            setting = "AppSettings::DeriveDisplayOrder",
            setting = "AppSettings::DontCollapseArgsInUsage",
        )
    )]
    /// Tally the number of crates that depend on a group of crates over time.
    Tally(Args),
}

#[derive(StructOpt)]
struct Args {
    /// Download tarball of crates.io metadata
    #[structopt(long = "init")]
    init: bool,

    /// Display line graph using gnuplot, rather than dump csv
    #[structopt(long = "graph", value_name = "TITLE")]
    title: Option<String>,

    /// Display as a fraction of total crates, not absolute number
    #[structopt(long = "relative")]
    relative: bool,

    /// Count transitive dependencies, not just direct dependencies
    #[structopt(long = "transitive")]
    transitive: bool,

    /// Ignore a dependency coming from any crates matching regex
    #[structopt(long = "exclude", value_name = "REGEX")]
    exclude: Option<String>,

    #[structopt(name = "CRATE")]
    crates: Vec<String>,
}

fn main() {
    let mut builder = env_logger::Builder::new();
    builder.format(|out, record| write!(out, "{}", record.args()));
    if let Ok(log_config) = env::var("TALLY_LOG") {
        builder.parse(&log_config);
    }
    builder.init();

    let mut config = match Config::default() {
        Ok(cfg) => cfg,
        Err(e) => {
            let mut shell = Shell::new();
            cargo::exit_with_error(e.into(), &mut shell)
        }
    };

    let app = Opts::clap();
    let matches = app.get_matches();
    let Opts::Tally(args) = Opts::from_clap(&matches);
    if !args.init && args.crates.is_empty() {
        Opts::from_iter(&["cargo", "tally", "--help"]);
        process::exit(1);
    }

    if let Err(e) = real_main(args, &mut config) {
        let mut shell = Shell::new();
        cargo::exit_with_error(e, &mut shell)
    }
}

fn real_main(args: Args, _config: &mut Config) -> CliResult {
    let result = if args.init { init() } else { tally(&args) };

    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            eprintln!("{}", err);
            Err(CliError::code(1))
        }
    }
}

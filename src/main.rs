#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::default_trait_access,
    clippy::doc_markdown,
    clippy::enum_glob_use,
    clippy::let_and_return,
    clippy::let_underscore_drop,
    clippy::manual_map, // https://github.com/rust-lang/rust-clippy/issues/6795
    clippy::map_entry,
    clippy::needless_pass_by_value,
    clippy::needless_return,
    clippy::unnecessary_unwrap,
    clippy::unreadable_literal
)]

use structopt::clap::AppSettings;
use structopt::StructOpt;

use std::env;
use std::io::{self, Write};
use std::process;

mod csv;
mod debug;
mod error;
mod graph;
mod init;
mod intern;
mod tally;

use crate::init::init;
use crate::tally::tally;

#[derive(StructOpt)]
#[structopt(bin_name = "cargo")]
enum Opts {
    #[structopt(
        name = "tally",
        setting = AppSettings::UnifiedHelpMessage,
        setting = AppSettings::DeriveDisplayOrder,
        setting = AppSettings::DontCollapseArgsInUsage
    )]
    /// Tally the number of crates that depend on a group of crates over time.
    Tally(Args),
}

#[derive(StructOpt)]
struct Args {
    /// Download dump of crates.io metadata
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
        builder.parse_filters(&log_config);
    }
    builder.init();

    let Opts::Tally(args) = Opts::from_args();
    if !args.init && args.crates.is_empty() {
        Opts::from_iter(&["cargo", "tally", "--help"]);
        process::exit(1);
    }

    let result = if args.init { init() } else { tally(&args) };

    if let Err(err) = result {
        let _ = writeln!(io::stderr(), "{}", err);
        process::exit(1);
    }
}

#![cfg_attr(
    feature = "cargo-clippy",
    allow(let_and_return, map_entry, needless_pass_by_value, redundant_closure_call)
)]

#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate log;

#[macro_use]
extern crate failure;

extern crate cargo;
extern crate cargo_tally;
extern crate chrono;
extern crate env_logger;
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

use cargo::core::shell::Shell;
use cargo::{CargoError, CliError, CliResult, Config};

use std::env;
use std::io::Write;

mod csv;
mod debug;
mod graph;
mod init;
mod intern;
mod progress;
mod tally;

use init::init;
use tally::tally;

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

    let result = (|| {
        let args: Vec<_> = try!(
            env::args_os()
                .map(|s| {
                    s.into_string().map_err(|s| {
                        CargoError::from(format_err!("invalid unicode in argument: {:?}", s))
                    })
                })
                .collect()
        );
        let rest = &args;
        cargo::call_main_without_stdin(real_main, &mut config, USAGE, rest, false)
    })();

    match result {
        Err(e) => cargo::exit_with_error(e, &mut *config.shell()),
        Ok(()) => {}
    }
}

fn real_main(flags: Flags, _config: &mut Config) -> CliResult {
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

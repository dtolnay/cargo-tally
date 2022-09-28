use crate::{cratename, user};
use clap::builder::{ArgAction, ValueParser};
use clap::{Arg, Command};
use regex::Regex;
use semver::VersionReq;
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::PathBuf;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug)]
pub(crate) struct Opt {
    pub db: PathBuf,
    pub exclude: Vec<Regex>,
    pub jobs: usize,
    pub relative: bool,
    pub title: Option<String>,
    pub transitive: bool,
    pub queries: Vec<String>,
}

const USAGE: &str = "\
    cargo tally [OPTIONS] QUERIES...
    cargo tally serde:1.0 'anyhow:^1.0 + thiserror'";

const TEMPLATE: &str = "\
{bin} {version}
David Tolnay <dtolnay@gmail.com>
https://github.com/dtolnay/cargo-tally

{usage-heading}
    {usage}

{all-args}\
";

fn app(jobs_help: &String) -> Command {
    let mut app = Command::new("cargo-tally")
        .override_usage(USAGE)
        .help_template(TEMPLATE)
        .arg(arg_db())
        .arg(arg_exclude())
        .arg(arg_jobs(jobs_help))
        .arg(arg_relative())
        .arg(arg_title())
        .arg(arg_transitive())
        .arg(arg_queries());
    if let Some(version) = option_env!("CARGO_PKG_VERSION") {
        app = app.version(version);
    }
    app
}

const DB: &str = "db";
const EXCLUDE: &str = "exclude";
const JOBS: &str = "jobs";
const RELATIVE: &str = "relative";
const TITLE: &str = "title";
const TRANSITIVE: &str = "transitive";
const QUERIES: &str = "queries";

pub(crate) fn parse() -> Opt {
    // | threads | duration | allocated |  peak   |
    // |---------|----------|-----------|---------|
    // |     1   |  38.6 s  |   55.2 GB | 11.0 GB |
    // |     2   |  24.8 s  |   55.4 GB | 10.2 GB |
    // |     4   |  14.2 s  |   55.8 GB |  8.8 GB |
    // |     8   |  12.7 s  |   58.4 GB |  8.3 GB |
    // |    16   |  12.6 s  |   59.2 GB |  8.2 GB |
    // |    32   |  12.8 s  |   63.2 GB |  8.4 GB |
    // |    64   |  14.0 s  |   69.5 GB | 11.1 GB |
    let default_jobs = num_cpus::get().min(32);
    let jobs_help = format!(
        "Number of threads to run differential dataflow [default: {}]",
        default_jobs,
    );

    let mut args: Vec<_> = env::args_os().collect();
    if let Some(first) = args.get_mut(0) {
        *first = OsString::from("cargo-tally");
    }
    if args.get(1).map(OsString::as_os_str) == Some(OsStr::new("tally")) {
        args.remove(1);
    }
    let matches = app(&jobs_help).get_matches_from(args);

    let db = PathBuf::from(matches.get_one::<PathBuf>(DB).unwrap());

    let exclude = matches
        .get_many::<Regex>(EXCLUDE)
        .unwrap_or_default()
        .cloned()
        .collect();

    let jobs = matches
        .get_one::<usize>(JOBS)
        .copied()
        .unwrap_or(default_jobs);

    let title = matches.get_one::<String>(TITLE).map(String::clone);

    let relative = matches.contains_id(RELATIVE);
    let transitive = matches.contains_id(TRANSITIVE);

    let queries = matches
        .get_many::<String>(QUERIES)
        .unwrap()
        .map(String::clone)
        .collect();

    Opt {
        db,
        exclude,
        jobs,
        relative,
        title,
        transitive,
        queries,
    }
}

fn arg_db() -> Arg {
    Arg::new(DB)
        .long(DB)
        .num_args(1)
        .value_name("PATH")
        .default_value("./db-dump.tar.gz")
        .value_parser(ValueParser::path_buf())
        .help("Path to crates.io's database dump")
}

fn arg_exclude() -> Arg {
    Arg::new(EXCLUDE)
        .long(EXCLUDE)
        .hide(true)
        .action(ArgAction::Append)
        .value_name("REGEX")
        .value_parser(Regex::from_str)
        .help("Ignore a dependency coming from any crates matching regex")
}

fn arg_jobs(help: &String) -> Arg {
    Arg::new(JOBS)
        .long(JOBS)
        .short('j')
        .num_args(1)
        .value_name("N")
        .value_parser(usize::from_str)
        .help(help)
}

fn arg_relative() -> Arg {
    Arg::new(RELATIVE)
        .long(RELATIVE)
        .num_args(0)
        .help("Display as a fraction of total crates, not absolute number")
}

fn arg_title() -> Arg {
    Arg::new(TITLE)
        .long(TITLE)
        .hide(true)
        .num_args(1)
        .value_name("TITLE")
        .value_parser(ValueParser::string())
        .help("Graph title")
}

fn arg_transitive() -> Arg {
    Arg::new(TRANSITIVE)
        .long(TRANSITIVE)
        .num_args(0)
        .help("Count transitive dependencies, not just direct dependencies")
}

fn arg_queries() -> Arg {
    Arg::new(QUERIES)
        .required(true)
        .num_args(0..)
        .value_name("QUERIES")
        .value_parser(validate_query)
        .help("Queries")
        .hide(true)
}

#[derive(Error, Debug)]
enum Error {
    #[error("invalid crates.io username")]
    InvalidUsername,
    #[error("invalid crate name according to crates.io")]
    InvalidCrateName,
    #[error(transparent)]
    Semver(#[from] semver::Error),
}

fn validate_query(string: &str) -> Result<String, Error> {
    for predicate in string.split('+') {
        let predicate = predicate.trim();

        if let Some(username) = predicate.strip_prefix('@') {
            if username.split('/').all(user::valid) {
                continue;
            } else {
                return Err(Error::InvalidUsername);
            }
        }

        let (name, req) = if let Some((name, req)) = predicate.split_once(':') {
            (name, Some(req))
        } else {
            (predicate, None)
        };

        if !cratename::valid(name.trim()) {
            return Err(Error::InvalidCrateName);
        }

        if let Some(req) = req {
            VersionReq::from_str(req)?;
        }
    }

    Ok(string.to_owned())
}

#[test]
fn test_cli() {
    let jobs_help = String::new();
    app(&jobs_help).debug_assert();
}

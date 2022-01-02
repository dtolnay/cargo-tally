use crate::{cratename, user};
use clap::{App, Arg};
use regex::Regex;
use semver::VersionReq;
use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
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

USAGE:
    {usage}

OPTIONS:
{options}\
";

fn app(jobs_help: &str) -> App {
    let mut app = App::new("cargo-tally")
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

    let db = PathBuf::from(matches.value_of_os(DB).unwrap());

    let exclude = matches
        .values_of(EXCLUDE)
        .unwrap_or_default()
        .map(|regex| regex.parse().unwrap())
        .collect();

    let jobs = matches
        .value_of(JOBS)
        .map_or(default_jobs, |jobs| jobs.parse().unwrap());

    let title = matches.value_of(TITLE).map(str::to_owned);

    let relative = matches.is_present(RELATIVE);
    let transitive = matches.is_present(TRANSITIVE);

    let queries = matches
        .values_of(QUERIES)
        .unwrap()
        .map(str::to_owned)
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

fn arg_db<'help>() -> Arg<'help> {
    Arg::new(DB)
        .long(DB)
        .takes_value(true)
        .value_name("PATH")
        .default_value("./db-dump.tar.gz")
        .allow_invalid_utf8(true)
        .help("Path to crates.io's database dump")
}

fn arg_exclude<'help>() -> Arg<'help> {
    Arg::new(EXCLUDE)
        .long(EXCLUDE)
        .hide(true)
        .multiple_occurrences(true)
        .value_name("REGEX")
        .validator_os(validate_parse::<Regex>)
        .help("Ignore a dependency coming from any crates matching regex")
}

fn arg_jobs<'help>(help: &'help str) -> Arg<'help> {
    Arg::new(JOBS)
        .long(JOBS)
        .short('j')
        .takes_value(true)
        .value_name("N")
        .validator_os(validate_parse::<usize>)
        .help(help)
}

fn arg_relative<'help>() -> Arg<'help> {
    Arg::new(RELATIVE)
        .long(RELATIVE)
        .help("Display as a fraction of total crates, not absolute number")
}

fn arg_title<'help>() -> Arg<'help> {
    Arg::new(TITLE)
        .long(TITLE)
        .hide(true)
        .takes_value(true)
        .value_name("TITLE")
        .validator_os(validate_parse::<String>)
        .help("Graph title")
}

fn arg_transitive<'help>() -> Arg<'help> {
    Arg::new(TRANSITIVE)
        .long(TRANSITIVE)
        .help("Count transitive dependencies, not just direct dependencies")
}

fn arg_queries<'help>() -> Arg<'help> {
    Arg::new(QUERIES)
        .required(true)
        .multiple_values(true)
        .value_name("QUERIES")
        .validator_os(validate_query)
        .help("Queries")
}

#[derive(Error, Debug)]
enum Error {
    #[error("invalid utf-8 sequence")]
    Utf8,
    #[error("invalid crates.io username")]
    InvalidUsername,
    #[error("invalid crate name according to crates.io")]
    InvalidCrateName,
    #[error(transparent)]
    Semver(#[from] semver::Error),
    #[error("{0}")]
    Msg(String),
}

fn validate_utf8(arg: &OsStr) -> Result<&str, Error> {
    arg.to_str().ok_or(Error::Utf8)
}

fn validate_parse<T>(arg: &OsStr) -> Result<T, Error>
where
    T: FromStr,
    T::Err: Display,
{
    validate_utf8(arg)?
        .parse::<T>()
        .map_err(|err| Error::Msg(err.to_string()))
}

fn validate_query(arg: &OsStr) -> Result<(), Error> {
    for predicate in validate_utf8(arg)?.split('+') {
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
    Ok(())
}

#[test]
fn test_cli() {
    app("").debug_assert();
}

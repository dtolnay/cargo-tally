use crate::{cratename, user};
use clap::builder::{ArgAction, TypedValueParser, ValueParser};
use clap::error::ErrorKind;
use clap::{Arg, Command};
use ghost::phantom;
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

fn app(jobs_help: &str) -> Command {
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

fn arg_db<'help>() -> Arg<'help> {
    Arg::new(DB)
        .long(DB)
        .takes_value(true)
        .value_name("PATH")
        .default_value("./db-dump.tar.gz")
        .value_parser(ValueParser::path_buf())
        .help("Path to crates.io's database dump")
}

fn arg_exclude<'help>() -> Arg<'help> {
    Arg::new(EXCLUDE)
        .long(EXCLUDE)
        .hide(true)
        .action(ArgAction::Append)
        .value_name("REGEX")
        .value_parser(ValueParserFromStr::<Regex>)
        .help("Ignore a dependency coming from any crates matching regex")
}

fn arg_jobs<'help>(help: &'help str) -> Arg<'help> {
    Arg::new(JOBS)
        .long(JOBS)
        .short('j')
        .takes_value(true)
        .value_name("N")
        .value_parser(ValueParserFromStr::<usize>)
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
        .value_parser(ValueParser::string())
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
        .value_parser(ValidateQuery)
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
}

#[phantom]
struct ValueParserFromStr<T>;

impl<T> Clone for ValueParserFromStr<T> {
    fn clone(&self) -> Self {
        ValueParserFromStr
    }
}

impl<T> TypedValueParser for ValueParserFromStr<T>
where
    T: Send + Sync + FromStr + 'static,
    T::Err: Display,
{
    type Value = T;

    fn parse_ref(
        &self,
        _cmd: &Command,
        _arg: Option<&Arg>,
        value: &OsStr,
    ) -> clap::Result<Self::Value> {
        let string = value
            .to_str()
            .ok_or_else(|| clap::Error::raw(ErrorKind::InvalidUtf8, Error::Utf8))?;
        T::from_str(string).map_err(|err| clap::Error::raw(ErrorKind::InvalidValue, err))
    }
}

#[derive(Clone)]
struct ValidateQuery;

impl TypedValueParser for ValidateQuery {
    type Value = String;

    fn parse_ref(
        &self,
        cmd: &Command,
        arg: Option<&Arg>,
        value: &OsStr,
    ) -> clap::Result<Self::Value> {
        self.parse(cmd, arg, value.to_owned())
    }

    fn parse(
        &self,
        _cmd: &Command,
        _arg: Option<&Arg>,
        value: OsString,
    ) -> clap::Result<Self::Value> {
        let string = value
            .into_string()
            .map_err(|_| clap::Error::raw(ErrorKind::InvalidUtf8, Error::Utf8))?;

        for predicate in string.split('+') {
            let predicate = predicate.trim();

            if let Some(username) = predicate.strip_prefix('@') {
                if username.split('/').all(user::valid) {
                    continue;
                } else {
                    return Err(clap::Error::raw(
                        ErrorKind::ValueValidation,
                        Error::InvalidUsername,
                    ));
                }
            }

            let (name, req) = if let Some((name, req)) = predicate.split_once(':') {
                (name, Some(req))
            } else {
                (predicate, None)
            };

            if !cratename::valid(name.trim()) {
                return Err(clap::Error::raw(
                    ErrorKind::ValueValidation,
                    Error::InvalidCrateName,
                ));
            }

            if let Some(req) = req {
                VersionReq::from_str(req)
                    .map_err(|err| clap::Error::raw(ErrorKind::ValueValidation, err))?;
            }
        }

        Ok(string)
    }
}

#[test]
fn test_cli() {
    app("").debug_assert();
}

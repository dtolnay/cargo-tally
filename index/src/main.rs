mod dir;
mod error;
mod json;

use chrono::{NaiveDateTime, Utc};
use flate2::Compression;
use flate2::write::GzEncoder;
use git2::{Commit, Repository};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use lazy_static::lazy_static;
use regex::Regex;
use semver::Version;
use structopt::StructOpt;

use std::collections::BTreeMap as Map;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process;

use crate::error::{Error, Result};
use crate::json::Crate;

const TIPS: [&str; 2] = ["origin/master", "origin/snapshot-2018-09-26"];

type DateTime = chrono::DateTime<Utc>;

#[derive(StructOpt, Debug)]
struct Opts {
    /// Path containing crates.io-index checkout
    #[structopt(value_name = "INDEX")]
    index: PathBuf,
}

fn main() {
    if let Err(err) = try_main() {
        let _ = writeln!(io::stderr(), "Error: {}", err);
        process::exit(1);
    }
}

fn try_main() -> Result<()> {
    let opts = Opts::from_args();
    let repo = Repository::open(&opts.index)?;
    let crates = parse_index(&opts.index)?;
    let pb = setup_progress_bar(crates.len());
    let timestamps = compute_timestamps(repo, &pb)?;
    let crates = consolidate_crates(crates, timestamps);
    write_json(crates)?;
    pb.finish_and_clear();
    Ok(())
}

fn parse_index(index: &Path) -> Result<Vec<Crate>> {
    let mut crates = Vec::new();

    for file in dir::crate_files(index)? {
        let content = fs::read(&file)?;
        let de = serde_json::Deserializer::from_slice(&content);

        for line in de.into_iter::<Crate>() {
            match line {
                Ok(line) => crates.push(line),
                Err(err) => return Err(Error::JsonLine(file, err)),
            }
        }
    }

    Ok(crates)
}

fn setup_progress_bar(len: usize) -> ProgressBar {
    let pb = ProgressBar::new(len as u64);
    let style = ProgressStyle::default_bar()
        .template("[{wide_bar:.cyan/blue}] {percent}%")
        .progress_chars("&&.");
    pb.set_style(style);
    pb.set_draw_target(ProgressDrawTarget::stderr());
    pb
}

type Timestamps = Map<(String, Version), DateTime>;

fn compute_timestamps(repo: Repository, pb: &ProgressBar) -> Result<Timestamps> {
    let mut timestamps = Map::new();

    for tip in &TIPS {
        let object = repo.revparse_single(tip)?;
        let mut commit = object.into_commit().unwrap();

        loop {
            pb.inc(1);
            match classify_commit(&commit) {
                CommitType::Update(name, version) => {
                    let seconds_from_epoch = commit.time().seconds();
                    let naive = NaiveDateTime::from_timestamp(seconds_from_epoch, 0);
                    let datetime = DateTime::from_utc(naive, Utc);

                    timestamps.insert((name, version), datetime);
                }
                CommitType::Yank | CommitType::Unyank | CommitType::Manual => {}
                CommitType::Initial => break,
            }
            commit = commit.parent(0)?;
        }
    }

    Ok(timestamps)
}

fn consolidate_crates(crates: Vec<Crate>, timestamps: Timestamps) -> Vec<Crate> {
    let mut crates: Vec<Crate> = crates
        .into_iter()
        .filter_map(|mut krate| {
            let key = (krate.name.clone(), krate.vers.clone());
            let timestamp = timestamps.get(&key)?;
            krate.published = Some(timestamp.clone());
            Some(krate)
        })
        .collect();

    fn sort_key(krate: &Crate) -> (&Option<DateTime>, &str, &Version) {
        (&krate.published, &krate.name, &krate.vers)
    }

    crates.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));

    crates
}

fn write_json(crates: Vec<Crate>) -> Result<()> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());

    for krate in crates {
        let line = serde_json::to_vec(&krate)?;
        encoder.write_all(&line)?;
        encoder.write_all(b"\n")?;
    }

    let gz = encoder.finish()?;
    fs::write("tally.json.gz", gz)?;
    Ok(())
}

#[derive(PartialEq)]
enum CommitType {
    Update(String, Version),
    Yank,
    Unyank,
    Manual,
    Initial,
}

fn classify_commit(commit: &Commit) -> CommitType {
    if commit.parent_count() == 0 {
        return CommitType::Initial;
    }

    let summary = match commit.summary() {
        Some(summary) => summary,
        None => return CommitType::Manual,
    };

    lazy_static! {
        static ref UPDATE: Regex = Regex::new(r"^Updating crate `([^#]+)#([^`]+)`$").unwrap();
    }

    if let Some(update) = UPDATE.captures(&summary) {
        let name = update[1].to_owned();
        let version = &update[2];
        match version.parse() {
            Ok(version) => CommitType::Update(name, version),
            Err(err) => panic!("unexpected version `{}`: {}", version, err),
        }
    } else if summary.starts_with("Yanking crate") {
        CommitType::Yank
    } else if summary.starts_with("Unyanking crate") {
        CommitType::Unyank
    } else {
        CommitType::Manual
    }
}

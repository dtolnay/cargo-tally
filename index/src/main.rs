mod dir;
mod error;
mod transitive;

use cargo_tally::{Crate, universe, intern::crate_name};
use chrono::{NaiveDateTime, Utc};
use flate2::write::{GzEncoder, GzDecoder};
use flate2::Compression;
use git2::{Commit, Repository};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use lazy_static::lazy_static;
use regex::Regex;
use semver::Version;
use structopt::StructOpt;

use std::cmp::Ordering;
use std::collections::BTreeMap as Map;
use std::fs;
use std::io::{self, Write, Read};
use std::path::{Path, PathBuf};
use std::process;

use crate::error::{Error, Result};

const TIPS: [&str; 2] = ["origin/master", "origin/snapshot-2018-09-26"];

type DateTime = chrono::DateTime<Utc>;
// 139_079 crates in crates.io
#[derive(StructOpt, Debug)]
struct Opts {
    /// Path containing crates.io-index checkout
    #[structopt(value_name = "INDEX")]
    index: PathBuf,
}

fn test() -> Result<Vec<Crate>> {
    let pb = setup_progress_bar(100_000);

    let json_path = Path::new("../tally.json");
    if !json_path.exists() {
        panic!("no file {:?}", json_path)
    }

    let json = std::fs::read(json_path)?;
    let de = serde_json::Deserializer::from_slice(&json);
    let mut ret = Vec::new();
    for line in pb.wrap_iter(de.into_iter::<Crate>()) {
        let krate = line?;
        ret.push(krate);
    }
    pb.finish_and_clear();
    Ok(ret)
}

fn load_computed() -> Result<Vec<Row>> {
    let pb = setup_progress_bar(100_000);
    let json_path = Path::new("./computed.json");
    if !json_path.exists() {
        panic!("no file {:?}", json_path)
    }

    let mut krates = Vec::new();
    // let file = fs::File::open(json_path)?;
    // let mut decoder = GzDecoder::new(file);
    // let mut decompressed = Vec::new();
    // decoder.read_to_end(&mut decompressed)?;
    let json = fs::read(json_path)?;
    let de = serde_json::Deserializer::from_slice(&json);
    for line in pb.wrap_iter(de.into_iter::<Row>()) {
        let krate = line?;
        krates.push(krate);
    }
    pb.finish_and_clear();
    
    Ok(krates)
}

// TODO ask about try_main
fn main() -> Result<()> {
    let opts = Opts::from_args();
    //let repo = Repository::open(&opts.index).expect("open rep");
    // let crates = parse_index(&opts.index).expect("parse idx");
    // let pb = setup_progress_bar(crates.len());
    // let timestamps = compute_timestamps(repo, &pb)?;
    // let crates = consolidate_crates(crates, timestamps);

    // let crates = test()?;
    //let pb = setup_progress_bar(crates.len());

    // let (uni, rows) = universe(crates, &pb);

    // for (k, v) in uni.reverse_depends.iter() {
    //     if k.name == crate_name("tar") {
    //         println!("{:?}: {}", k, v.len());
    //     }
    //     //println!("{:?}: {}", k, v.len());
    // }
    // println!();
    // for row in rows.iter() {
    //     if row.name == crate_name("tar") {
    //         println!("{:?}: {}", row, row.counts);
    //     }
    // }
    let table = load_computed()?
        .into_iter()
        .filter(|k| k.name == crate_name("tar"))
        .collect::<Vec<_>>();
    //write_json(cargo_tally::JSONFILE, crates)?;
    // or make a new function
    draw_graph("tar", table.as_ref());
    // write_json(cargo_tally::COMPFILE, rows)?;
    
    //pb.finish_and_clear();
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

#[derive(Eq)]
struct Key {
    name: String,
    version: Version,
}

impl Ord for Key {
    fn cmp(&self, other: &Self) -> Ordering {
        self.name
            .cmp(&other.name)
            .then_with(|| self.version.cmp(&other.version))
            .then_with(|| self.version.build.cmp(&other.version.build))
    }
}

impl PartialOrd for Key {
    fn partial_cmp(&self, other: &Key) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for Key {
    fn eq(&self, other: &Key) -> bool {
        self.name == other.name
            && self.version == other.version
            && self.version.build == other.version.build
    }
}

type Timestamps = Map<Key, DateTime>;

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

                    let key = Key { name, version };
                    timestamps.entry(key).or_insert(datetime);
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
            let key = Key {
                name: krate.name.clone(),
                version: krate.version.clone(),
            };
            let timestamp = timestamps.get(&key)?;
            krate.published = Some(timestamp.clone());
            Some(krate)
        })
        .collect();

    fn sort_key(krate: &Crate) -> (&Option<DateTime>, &str, &Version) {
        (&krate.published, &krate.name, &krate.version)
    }

    crates.sort_by(|a, b| sort_key(a).cmp(&sort_key(b)));

    crates
}

// fn compute_transitive(crates: &[Crate], pb: &ProgressBar) -> Vec<TranitiveCrateDeps> {
//     let mut ret = Vec::new();
//     for krate in crates.iter().take(1) {
//         let t_dep = TranitiveCrateDeps::calc_dependencies(crates, krate, pb);
//         // println!("{:?}", t_dep);
//         ret.push(t_dep);
        
//         //pb.inc(1);
//     }

//     // TODO remove
//     for dep in ret.iter() {
//         println!("name: {} count: {}", dep.name, dep.depended_on.len());
//     }
//     if let Some(tokio) = ret.iter().find(|d| d.name == "tokio") {
//         println!("name: {} count: {}", tokio.name, tokio.depended_on.len());
//     }
//     ret
// }

fn write_json<T: serde::Serialize>(file: &str, crates: Vec<T>) -> Result<()> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());

    for krate in crates {
        let line = serde_json::to_vec(&krate)?;
        encoder.write_all(&line)?;
        encoder.write_all(b"\n")?;
    }

    let gz = encoder.finish()?;
    fs::write(file, gz)?;
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


use gnuplot::{
    AlignLeft, AlignTop, Auto, AxesCommon, Color, Figure, Fix, Graph, LineWidth,
    MajorScale, Placement,
};
use chrono::{NaiveDate, NaiveTime};
use palette;
use palette::{Hue, Srgb};

use cargo_tally::Row;

fn draw_graph(args: &str, table: &[Row]) {
    let mut colors = Vec::new();
    let primary: palette::Color = Srgb::new(217u8, 87, 43).into_format().into_linear().into();
    let n = 1;
    for i in 0..n {
        let linear = primary.shift_hue(360.0 * ((i + 1) as f32) / (n as f32));
        let srgb = Srgb::from_linear(linear.into()).into_format::<u8>();
        let hex = format!("#{:02X}{:02X}{:02X}", srgb.red, srgb.green, srgb.blue);
        colors.push(hex);
        //captions.push(&args.crates[i].replace('_', "\\\\_"));
    }

    let mut fg = Figure::new();
    {
        // Create plot
        let axes = fg.axes2d();
        axes.set_title("testing tar transitive deps", &[]);
        axes.set_x_range(
            Fix(float_year(&table[0].timestamp) - 0.3),
            Fix(float_year(&Utc::now()) + 0.15),
        );
        axes.set_y_range(Fix(0.0), Auto);
        axes.set_x_ticks(Some((Fix(1.0), 12)), &[MajorScale(2.0)], &[]);
        axes.set_legend(
            Graph(0.05),
            Graph(0.9),
            &[Placement(AlignLeft, AlignTop)],
            &[],
        );

        // Create x-axis
        let mut x = Vec::new();
        for row in table {
            x.push(float_year(&row.timestamp));
        }

        // Create series
        for i in 0..n {
            let mut y = Vec::new();
            for row in table {
                y.push(row.counts);
            }
            axes.lines(
                &x,
                &y,
                &[LineWidth(1.5), Color(&colors[i])],
            );
        }
    }
    fg.show();
}
fn float_year(dt: &DateTime) -> f64 {
    let nd = NaiveDate::from_ymd(2017, 1, 1);
    let nt = NaiveTime::from_hms_milli(0, 0, 0, 0);
    let base = DateTime::from_utc(NaiveDateTime::new(nd, nt), Utc);
    let offset = dt.signed_duration_since(base);
    let year = offset.num_minutes() as f64 / 525_960.0 + 2017.0;
    year
}

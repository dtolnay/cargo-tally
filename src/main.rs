#![allow(
    clippy::let_and_return,
    clippy::map_entry,
    clippy::needless_pass_by_value,
    clippy::unreadable_literal
)]

use structopt::clap::AppSettings;
use structopt::StructOpt;

use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Utc};
use semver::Version;
use serde::{Deserialize, Serialize};
use indicatif::ProgressBar;
use flate2::read::GzDecoder;
use rayon::prelude::*;
use gnuplot::{
    AlignLeft, AlignTop, Auto, AxesCommon, Caption, Color, Figure, Fix, Graph, LineWidth,
    MajorScale, Placement,
};
use palette;
use palette::{Hue, Srgb};

use std::env;
use std::fs;
use std::io::{self, Write, Read};
use std::path::Path;
use std::process;

mod csv;
mod debug;
mod error;
mod graph;
mod init;
mod intern;
mod progress;
mod tally;

use crate::init::init;
use crate::tally::tally;

type DateTime = chrono::DateTime<Utc>;

#[derive(StructOpt)]
#[structopt(bin_name = "cargo")]
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

// TODO This is copied from pre_calc struct is serialized in that crate and 
// deserialized here can't import from that crate, I'm sure there 
// is a better way to do this.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TranitiveDep {
    pub name: String,
    pub timestamp: DateTime,
    pub version: Version,
    pub transitive_count: usize,
    pub direct_count: usize,
    pub total: usize,
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

// TODO fix errors so uses error.rs errors
fn load_computed(pb: &ProgressBar) -> Result<Vec<TranitiveDep>, io::Error> {
    let json_path = Path::new("../computed.json.gz");
    if !json_path.exists() {
        panic!("no file {:?}", json_path)
    }
 
    let file = fs::File::open(json_path)?;
    let mut decoder = GzDecoder::new(file);
    let mut decompressed = String::new();
    decoder.read_to_string(&mut decompressed)?; 

    let mut krates = decompressed
        .par_lines()
        .map(|line| {
            serde_json::from_str(line)
            .map_err(|e| {
                panic!("{:?}", e)
            })
            .unwrap()
        })
        .collect::<Vec<TranitiveDep>>();
    // let de = serde_json::Deserializer::from_slice(&decompressed);
    // let mut krates = Vec::new();
    // for line in pb.wrap_iter(de.into_iter::<TransitiveDep>()) {
    //     let krate = line?;
    //     krates.push(krate);
    // }
    pb.finish_and_clear();

    krates.par_sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok(krates)
}

// fn matching_crates(krate: &TranitiveDep) -> bool {

// }

// fn tally2(args: &Args, pb: &ProgressBar) -> Result<(), io::Error> {
//     let table = load_computed(&pb)?
//         .into_par_iter()
//         .inspect(|_| pb.inc(1))
//         .filter(matching_crates)
//         .collect::<Vec<_>>();
//     draw_graph2(args, table.as_ref());
//     Ok(())
// }

fn draw_graph2(args: &Args, table: &[TranitiveDep]) {
    let mut colors = Vec::new();
    let mut captions = Vec::new();
    let primary: palette::Color = Srgb::new(217u8, 87, 43).into_format().into_linear().into();
    let n = args.crates.len();
    for i in 0..n {
        let linear = primary.shift_hue(360.0 * ((i + 1) as f32) / (n as f32));
        let srgb = Srgb::from_linear(linear.into()).into_format::<u8>();
        let hex = format!("#{:02X}{:02X}{:02X}", srgb.red, srgb.green, srgb.blue);
        colors.push(hex);
        captions.push(args.crates[i].replace('_', "\\\\_"));
    }

    let mut fg = Figure::new();
    {
        // Create plot
        let axes = fg.axes2d();
        axes.set_title(&args.title.as_ref().unwrap().replace('_', "\\\\_"), &[]);
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
            if args.relative {
                let mut y = Vec::new();

                for row in table {
                    let counts = if args.transitive { row.transitive_count } else { row.direct_count };
                    y.push(counts as f32 / row.total as f32);
                }
                axes.lines(
                    &x,
                    &y,
                    &[Caption(&captions[i]), LineWidth(1.5), Color(&colors[i])],
                );
            } else {
                let mut y = Vec::new();
                for row in table {
                    let counts = if args.transitive { row.transitive_count } else { row.direct_count };
                    y.push(counts);
                }
                axes.lines(
                    &x,
                    &y,
                    &[Caption(&captions[i]), LineWidth(1.5), Color(&colors[i])],
                );
            }
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

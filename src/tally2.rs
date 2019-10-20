use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Utc};
use flate2::read::GzDecoder;
use gnuplot::{
    AlignLeft, AlignTop, Auto, AxesCommon, Caption, Color, Figure, Fix, Graph, LineWidth,
    MajorScale, Placement,
};
use indicatif::ProgressBar;
use palette;
use palette::{Hue, Srgb};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};

use std::fs;
use std::io::{self, Read};
use std::path::Path;

use crate::error;
use crate::Args;
use crate::csv::print_csv;

type DateTime = chrono::DateTime<Utc>;

// TODO This is copied from pre_calc struct is serialized in that crate and 
// deserialized here can't import from that crate, I'm sure there 
// is a better way to do this.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransitiveDep {
    pub name: String,
    pub timestamp: DateTime,
    pub version: Version,
    pub transitive_count: usize,
    pub direct_count: usize,
    pub total: usize,
}

#[derive(Debug)]
struct Matcher<'a> {
    name: &'a str,
    req: VersionReq,
    nodes: Vec<u32>,
}


/// Returns time sorted `Vec<TransitiveDep>`  
fn load_computed(pb: &ProgressBar) -> Result<Vec<TransitiveDep>, error::Error> {
    let json_path = Path::new(cargo_tally::JSONFILE);
    if !json_path.exists() {
        return Err(error::Error::MissingJson);
    }
    pb.inc(1);

    let file = fs::File::open(json_path)?;
    let mut buf = std::io::BufReader::new(file);
    let mut decoder = GzDecoder::new(buf);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed)?;
    let de = serde_json::Deserializer::from_slice(&decompressed);

    let mut krates = Vec::new();
    for line in de.into_iter::<TransitiveDep>() {
        let k = line?;
        krates.push(k);
    }
    pb.finish_and_clear();
    Ok(krates)
}

fn create_matchers(search: &str) -> error::Result<Matcher> {
    let mut pieces = search.splitn(2, ':');
    let matcher = Matcher {
        name: pieces.next().unwrap(),
        req: match pieces.next().unwrap_or("*").parse() {
            Ok(req) => req,
            Err(err) => return Err(error::Error::ParseSeries(search.to_string(), err)),
        },
        nodes: Vec::new(),
    };
    Ok(matcher)
}

fn matching_crates(krate: &TransitiveDep, search: &[String]) -> bool {
    search.iter()
        .map(|s| create_matchers(s).expect("failed to parse"))
        .any(|matcher| matcher.name == krate.name && matcher.req.matches(&krate.version))
}

fn draw_graph2(args: &Args, table: &[TransitiveDep]) {
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
        // Create series
        for i in 0..n {
            let mut y = Vec::new();
            let mut x = Vec::new();
            for row in table {
                x.push(float_year(&row.timestamp));
                y.push(row.transitive_count);
            }
            axes.lines(
                &x,
                &y,
                &[Caption(&captions[i]), LineWidth(1.5), Color(&colors[i])],
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

pub(crate) fn tally2(args: &Args) -> error::Result<()> {
    // TODO prgressBar needs to have an actual len
    let pb = ProgressBar::new(139_079);
    pb.set_message("FIX ME");

    let table = load_computed(&pb)?
        .into_iter()
        .filter(|k| matching_crates(k, &args.crates))
        .collect::<Vec<_>>();
    
    if args.title.is_some() {
        draw_graph2(args, &table);
    } else {
        // TODO
        // print_csv(args, &table);
    }
    Ok(())
}

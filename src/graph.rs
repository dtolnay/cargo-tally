use chrono::{NaiveDate, NaiveDateTime, NaiveTime, Utc};

use gnuplot::{
    AlignLeft, AlignTop, Auto, AxesCommon, Caption, Color, Figure, Fix, Graph, LineWidth,
    MinorScale, Placement,
};

use palette;
use palette::{Hue, Srgb};

use crate::tally::Row;
use crate::Args;
use cargo_tally::DateTime;

pub(crate) fn draw_graph(args: &Args, table: &[Row]) {
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
        axes.set_x_ticks(Some((Fix(1.0), 12)), &[MinorScale(2.0)], &[]);
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
                    y.push(row.counts[i] as f32 / row.total as f32);
                }
                axes.lines(
                    &x,
                    &y,
                    &[Caption(&captions[i]), LineWidth(1.5), Color(&colors[i])],
                );
            } else {
                let mut y = Vec::new();
                for row in table {
                    y.push(row.counts[i]);
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
    let year = offset.num_minutes() as f64 / 525960.0 + 2017.0;
    year
}

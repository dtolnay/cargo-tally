use crate::total::Total;
use anyhow::Result;
use cargo_tally::matrix::Matrix;
use cargo_tally::timestamp::NaiveDateTime;
use std::env;
use std::fmt::{self, Display};
use std::fs;
use std::path::PathBuf;

pub(crate) fn graph(
    title: Option<&str>,
    transitive: bool,
    results: &Matrix,
    labels: &[String],
    total: Option<&Total>,
) -> Result<PathBuf> {
    let now = NaiveDateTime::now();

    let relative = total.is_some();
    let title = if let Some(title) = title {
        title
    } else if relative {
        if transitive {
            "fraction of crates.io depending transitively"
        } else {
            "fraction of crates.io depending directly"
        }
    } else {
        if transitive {
            "number of crates depending transitively"
        } else {
            "number of crates depending directly"
        }
    };

    let mut data = String::new();
    data += "[\n";
    for (i, label) in labels.iter().enumerate() {
        data += "      {\"name\":\"";
        data += label;
        data += "\", \"values\":[\n";
        let mut prev = None;
        for (timestamp, row) in results {
            let value = row[i];
            if prev.is_none() {
                if value == 0 {
                    continue;
                }
                let mut secs = timestamp.seconds();
                if timestamp.subsec_nanos() == 0 {
                    secs = secs.saturating_sub(1);
                }
                let timestamp = NaiveDateTime::from_timestamp(secs, 0);
                data += &Row(timestamp, 0, total).to_string();
            } else if prev == Some(value) {
                continue;
            }
            data += &Row(timestamp, value, total).to_string();
            prev = Some(value);
        }
        let (timestamp, last) = results.iter().next_back().unwrap();
        if timestamp < now {
            data += &Row(now, last[i], total).to_string();
        }
        data += "      ]},\n";
    }
    data += "    ]";

    let template = include_str!("index.html");
    let mut preprocessor_context = minipre::Context::new();
    preprocessor_context
        .define("CARGO_TALLY_TITLE", format!("\"{}\"", title.escape_debug()))
        .define("CARGO_TALLY_DATA", data)
        .define("CARGO_TALLY_RELATIVE", (relative as usize).to_string());
    let html = minipre::process_str(template, &mut preprocessor_context)?;

    let dir = env::temp_dir().join("cargo-tally");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.html", now.millis()));
    fs::write(&path, html)?;
    Ok(path)
}

struct Row<'a>(NaiveDateTime, u32, Option<&'a Total>);

impl<'a> Display for Row<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("        {\"time\":")?;
        write!(formatter, "{}", self.0.millis())?;
        formatter.write_str(", \"edges\":")?;
        if let Some(total) = self.2 {
            let total = total.eval(self.0);
            if total == 0 {
                formatter.write_str("0")?;
            } else {
                let fraction = self.1 as f32 / total as f32;
                write!(formatter, "{}", fraction)?;
            }
        } else {
            write!(formatter, "{}", self.1)?;
        }
        formatter.write_str("},\n")?;
        Ok(())
    }
}

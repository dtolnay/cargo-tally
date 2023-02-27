#![allow(non_upper_case_globals)]
#![allow(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::collapsible_else_if,
    clippy::expl_impl_clone_on_copy,
    clippy::let_underscore_untyped,
    clippy::manual_range_contains,
    clippy::module_name_repetitions,
    clippy::needless_lifetimes,
    clippy::redundant_else,
    clippy::single_match_else,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::uninlined_format_args,
    clippy::zero_prefixed_literal
)]

#[macro_use]
mod macros;

mod alloc;
mod args;
mod clean;
mod cratemap;
mod cratename;
mod filter;
mod load;
mod log;
mod mend;
mod query;
mod render;
mod total;
mod trace;
mod user;

use crate::load::load;
use crate::log::Log;
use crate::total::Total;
use anyhow::Result;
use atty::Stream;
use std::io::{self, Write};
use std::process;
use std::time::Instant;
use sysinfo::SystemExt;
use termcolor::{ColorChoice, StandardStream};

cargo_subcommand_metadata::description!(
    "Draw graphs of the number of dependencies on a crate over time"
);

fn main() {
    let mut stderr = StandardStream::stderr(ColorChoice::Auto);
    if let Err(err) = try_main(&mut stderr) {
        writeln!(stderr.error(), "{}", err);
        process::exit(1);
    }
}

fn try_main(stderr: &mut StandardStream) -> Result<()> {
    let opt = args::parse();

    if !opt.db.is_file() {
        write!(stderr.error(), "Database dump file does not exist: ");
        write!(stderr.red(), "{}", opt.db.display());
        let _ = writeln!(
            stderr,
            "\nDownload one from https://static.crates.io/db-dump.tar.gz",
        );
        process::exit(1);
    }

    let mut sysinfo = sysinfo::System::new();
    sysinfo.refresh_memory();
    let total_memory = sysinfo.total_memory();
    let (min_memory, advised) = if opt.transitive {
        (10 * 1024 * 1024 * 1024, "12 GB")
    } else {
        (7 * 1024 * 1024 * 1024, "8 GB")
    };
    if total_memory < min_memory && total_memory > 0 {
        writeln!(
            stderr.warning(),
            "warning: running with <{advised} memory is not advised.",
        );
    }

    let stdout_isatty = atty::is(Stream::Stdout);
    let stderr_isatty = atty::is(Stream::Stderr);

    let instant = Instant::now();
    let (mut db_dump, crates) = crate::load(&opt.db)?;
    crate::filter::filter(&mut db_dump, &crates, &opt.exclude);
    crate::mend::mend(&mut db_dump, &crates);
    db_dump.releases.sort_by_key(|v| v.created_at);
    crate::clean::clean(&mut db_dump, &crates);
    let total = opt.relative.then(|| Total::index(&db_dump.releases));
    if stderr_isatty {
        writeln!(stderr.trace(), "load time: {:.2?}", instant.elapsed());
    }

    let query_strings = opt.queries.iter().map(String::as_str);
    let queries = query::parse(query_strings, &crates)?;
    let instant = Instant::now();
    let results = cargo_tally::run(db_dump, opt.jobs, opt.transitive, &queries);
    if stderr_isatty {
        writeln!(stderr.trace(), "dataflow time: {:.2?}", instant.elapsed());
    }

    let _ = stderr.flush();
    let len = results.len();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    for (i, (timestamp, data)) in results.iter().enumerate() {
        if stdout_isatty && 10 + i == len && len > 20 {
            let _ = writeln!(stdout, "...");
        }
        if !stdout_isatty || i < 10 || 10 + i >= len {
            if let Some(total) = &total {
                let total = total.eval(timestamp);
                let _ = writeln!(stdout, "{:?} {:?}", timestamp, data / total);
            } else {
                let _ = writeln!(stdout, "{:?} {:?}", timestamp, data);
            }
        }
    }
    let _ = stdout.flush();

    if stdout_isatty {
        if results.is_empty() {
            writeln!(stderr.red(), "zero results");
        } else {
            let labels = opt
                .queries
                .iter()
                .map(|query| query::format(query, &crates))
                .collect::<Vec<_>>();
            let path = render::graph(
                opt.title.as_deref(),
                opt.transitive,
                &results,
                &labels,
                total.as_ref(),
            )?;
            if opener::open(&path).is_err() && stderr_isatty {
                let _ = writeln!(stderr, "graph written to {}", path.display());
            }
        }
    }

    if stderr_isatty {
        writeln!(stderr.trace(), "{}", alloc::stat());
    }

    Ok(())
}

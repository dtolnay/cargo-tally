use atty::{self, Stream::Stderr};
use failure::{self, Error};
use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};
use reqwest::{self, Response};

use std::fs::File;
use std::io;

use crate::progress::ProgressRead;

pub(crate) fn init() -> Result<(), Error> {
    //let snapshot = "https://github.com/dtolnay/cargo-tally/releases/download/2018-10-24/tally.json.gz";
    let snapshot = "https://github.com/dtolnay/cargo-tally/files/2924389/tally.json.gz";
    let jsongz = reqwest::get(snapshot)?.error_for_status()?;

    let pb = ProgressBar::hidden();
    if atty::is(Stderr) {
        setup_pb(&pb, &jsongz);
    }

    let mut tracker = ProgressRead::new(&pb, jsongz);
    let mut out = File::create("tally.json.gz")?;
    io::copy(&mut tracker, &mut out)?;

    pb.finish_and_clear();
    Ok(())
}

fn setup_pb(pb: &ProgressBar, resp: &Response) {
    let content_length = match resp.headers().get("Content-Length") {
        Some(header) => header,
        None => return,
    };

    let s = match content_length.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };

    let n = match s.parse() {
        Ok(n) => n,
        Err(_) => return,
    };

    pb.set_length(n);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("&&."),
    );
    pb.set_draw_target(ProgressDrawTarget::stderr());
}

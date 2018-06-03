use failure::{self, Error};

use flate2::read::GzDecoder;

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

use isatty::stderr_isatty;

use progress::ProgressRead;

use reqwest;
use reqwest::header::ContentLength;

use tar::Archive;

use unindent::unindent;

use std::env;
use std::path::Path;

pub(crate) fn init() -> Result<(), Error> {
    let tally_path = Path::new("tally");
    if tally_path.exists() {
        let pwd = env::current_dir().unwrap_or_else(|_| Path::new(".").to_owned());
        let helpful_path = pwd.join(tally_path);

        return Err(failure::err_msg(unindent(&format!(
            "
            Already exists: {}
            Remove and run `cargo tally --init` again.\
        ",
            helpful_path.display()
        ))));
    }

    let snapshot = "https://github.com/dtolnay/cargo-tally/releases/download/2018-06-02/tally.tgz";
    let tgz = reqwest::get(snapshot)?.error_for_status()?;

    let pb = ProgressBar::hidden();
    if stderr_isatty() {
        if let Some(&ContentLength(n)) = tgz.headers().get() {
            pb.set_length(n);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template(
                        "[{elapsed_precise}] [{wide_bar:.cyan/blue}] {bytes}/{total_bytes} ({eta})",
                    )
                    .progress_chars("&&."),
            );
            pb.set_draw_target(ProgressDrawTarget::stderr());
        }
    }

    let tracker = ProgressRead::new(&pb, tgz);
    let decoder = GzDecoder::new(tracker);
    let mut archive = Archive::new(decoder);
    archive.unpack(".")?;

    pb.finish_and_clear();
    Ok(())
}

extern crate cargo_tally;
use cargo_tally::*;

extern crate rayon;
use rayon::Scope;

use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};

extern crate indicatif;
use indicatif::{ProgressBar, ProgressStyle};

mod unwrap;
use unwrap::DisplayUnwrap;

const THREADS: usize = 24;

fn main() {
    env::set_var("ALLOW_DOWNLOAD", "");
    let config = rayon::Configuration::new().num_threads(THREADS);
    rayon::initialize(config).unwrap();

    let len = total_crates().display_unwrap();
    let pb = ProgressBar::new(len as u64);
    let len = AtomicUsize::new(len);

    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{wide_bar:.cyan/blue}] {percent}% elapsed: {elapsed_precise} eta: {eta_precise}")
            .progress_chars("&&."),
    );

    rayon::scope(|s| init_index(s, &len, &pb));
    pb.finish_and_clear();
}

fn init_index<'a>(s: &Scope<'a>, len: &'a AtomicUsize, pb: &'a ProgressBar) {
    let npages = num_pages().display_unwrap();
    for p in 1..npages + 1 {
        s.spawn(move |s| {
            let page = cache_index(p).display_unwrap();
            init_page(s, &page, len, pb);
        });
    }
}

fn init_page<'a>(s: &Scope<'a>, page: &IndexPage, len: &'a AtomicUsize, pb: &'a ProgressBar) {
    for krate in &page.crates {
        let name = krate.name.clone();
        s.spawn(move |s| {
            let k = cache_crate(&name).display_unwrap();
            let n = k.versions.len();
            pb.set_length((len.fetch_add(n, Ordering::SeqCst) + n) as u64);
            init_crate(s, &k, pb);
            pb.inc(1);
        });
    }
}

fn init_crate<'a>(s: &Scope<'a>, k: &Crate, pb: &'a ProgressBar) {
    for version in &k.versions {
        let name = k.index.name.clone();
        let num = version.num.clone();
        s.spawn(move |_s| {
            cache_dependencies(&name, &num).display_unwrap();
            pb.inc(1);
        });
    }
}

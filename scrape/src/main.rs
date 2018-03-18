extern crate cargo_tally;
use cargo_tally::*;

extern crate rayon;
use rayon::Scope;

use std::env;

extern crate indicatif;
use indicatif::{ProgressBar, ProgressStyle};

mod unwrap;
use unwrap::DisplayUnwrap;

const THREADS: usize = 24;

fn main() {
    env::set_var("ALLOW_DOWNLOAD", "");
    let config = rayon::Configuration::new().num_threads(THREADS);
    rayon::initialize(config).unwrap();
    let pb = ProgressBar::new(total_crates().unwrap() as u64);

    pb.set_style(
        ProgressStyle::default_bar()
            .template("[{wide_bar:.cyan/blue}] {percent}% elapsed: {elapsed_precise} eta: {eta_precise}")
            .progress_chars("&&."),
    );

    rayon::scope(|s| init_index(s, &pb));
    pb.finish_and_clear();
}

fn init_index<'a>(s: &Scope<'a>, pb: &'a ProgressBar) {
    let npages = num_pages().display_unwrap();
    for p in 1..npages + 1 {
        s.spawn(move |s| {
            let page = cache_index(p).display_unwrap();
            init_page(s, &page, pb);
        });
    }
}

fn init_page<'a>(s: &Scope<'a>, page: &IndexPage, pb: &'a ProgressBar) {
    for krate in &page.crates {
        let name = krate.name.clone();
        s.spawn(move |s| {
            let k = cache_crate(&name).display_unwrap();
            pb.inc(1);
            init_crate(s, &k);
        });
    }
}

fn init_crate(s: &Scope, k: &Crate) {
    for version in &k.versions {
        let name = k.index.name.clone();
        let num = version.num.clone();
        s.spawn(move |_s| {
            cache_dependencies(&name, &num).display_unwrap();
        });
    }
}

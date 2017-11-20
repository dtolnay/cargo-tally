extern crate cargo_tally;
use cargo_tally::*;

extern crate rayon;
use rayon::Scope;

use std::env;
use std::fmt::Display;

const THREADS: usize = 24;

fn main() {
    env::set_var("ALLOW_DOWNLOAD", "");
    let config = rayon::Configuration::new().num_threads(THREADS);
    rayon::initialize(config).unwrap();
    rayon::scope(init_index);
}

fn init_index<'scope>(s: &Scope<'scope>) {
    let npages = num_pages().display_unwrap();
    for p in 1..npages + 1 {
        s.spawn(move |s| {
            let page = cache_index(p).display_unwrap();
            init_page(s, &page);
        });
    }
}

fn init_page<'scope>(s: &Scope<'scope>, page: &IndexPage) {
    for krate in &page.crates {
        let name = krate.name.clone();
        s.spawn(move |s| {
            let k = cache_crate(&name).display_unwrap();
            init_crate(s, &k);
        });
    }
}

fn init_crate<'scope>(s: &Scope<'scope>, k: &Crate) {
    for version in &k.versions {
        let name = k.index.name.clone();
        let num = version.num.clone();
        s.spawn(move |_s| {
            cache_dependencies(&name, &num).display_unwrap();
        });
    }
}

trait DisplayUnwrap {
    type Output;
    fn display_unwrap(self) -> Self::Output;
}

impl<T, E> DisplayUnwrap for Result<T, E>
    where E: Display
{
    type Output = T;

    fn display_unwrap(self) -> T {
        self.unwrap_or_else(|err| panic!(err.to_string()))
    }
}

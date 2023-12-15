use std::env;
use std::fs;
use std::path::Path;
use std::process;

const CARGO_TALLY_MEMORY_LIMIT: &str = "CARGO_TALLY_MEMORY_LIMIT";

fn main() {
    let limit = if let Some(value) = env::var_os(CARGO_TALLY_MEMORY_LIMIT) {
        let Some(value) = value.to_str() else {
            eprintln!("failed to parse ${CARGO_TALLY_MEMORY_LIMIT}");
            process::exit(1);
        };
        let value = match value.parse::<u64>() {
            Ok(int) => int,
            Err(err) => {
                eprintln!("failed to parse ${CARGO_TALLY_MEMORY_LIMIT}: {err}");
                process::exit(1);
            }
        };
        Some(value)
    } else {
        None
    };

    let out_dir = env::var_os("OUT_DIR").unwrap();
    let out = Path::new(&out_dir).join("limit.mem");
    fs::write(out, format!("{limit:?}\n")).unwrap();

    println!("cargo:rerun-if-env-changed={CARGO_TALLY_MEMORY_LIMIT}");
}

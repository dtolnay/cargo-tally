use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn crate_files(index: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();

    for entry in fs::read_dir(index)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let name = entry.file_name();
        if name == ".git" {
            continue;
        }

        for entry in fs::read_dir(entry.path())? {
            let entry = entry?;

            if entry.file_type()?.is_file() {
                paths.push(entry.path());
                continue;
            }

            for entry in fs::read_dir(entry.path())? {
                let entry = entry?;

                paths.push(entry.path());
            }
        }
    }

    paths.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    Ok(paths)
}

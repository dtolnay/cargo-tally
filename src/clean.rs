use cargo_tally::arena::Slice;
use cargo_tally::cratemap::CrateMap;
use cargo_tally::version::Version;
use cargo_tally::DbDump;
use semver::{Comparator, Op};
use std::cmp;
use std::collections::btree_map::{BTreeMap as Map, Entry};

pub(crate) fn clean(db_dump: &mut DbDump, crates: &CrateMap) {
    let mut crate_max_version = Map::new();
    let mut dependencies_per_version = Map::new();

    for dep in &mut db_dump.dependencies {
        dependencies_per_version
            .entry(dep.version_id)
            .or_insert_with(Vec::new)
            .push(dep);
    }

    for rel in &db_dump.releases {
        match crate_max_version.entry(rel.crate_id) {
            Entry::Vacant(entry) => {
                entry.insert(&rel.num);
            }
            Entry::Occupied(entry) => {
                let entry = entry.into_mut();
                *entry = cmp::max(entry, &rel.num);
            }
        }

        let mut no_dependencies = Vec::new();
        let dependencies = dependencies_per_version
            .get_mut(&rel.id)
            .unwrap_or(&mut no_dependencies);
        let mut i = 0;
        while let Some(dep) = dependencies.get_mut(i) {
            if !crate_max_version.contains_key(&dep.crate_id) {
                // If every published version of a crate is a prerelease, Cargo
                // will resolve a `*` wildcard dependency to the max prerelease,
                // which we don't track.
                //
                // Other times, crates just go missing from the index, maybe for
                // legal reasons or because of leaked secrets.
                // https://github.com/rust-lang/crates.io-index/commit/a95f8bff541de7461638b5e4f75ee58747829ea3
                if crate::trace::VERBOSE {
                    eprintln!(
                        "unresolved dep {} {} on {} {}",
                        crates.name(rel.crate_id).unwrap(),
                        rel.num,
                        crates.name(dep.crate_id).unwrap(),
                        dep.req,
                    );
                }
                dependencies.remove(i);
                continue;
            }
            let max_version = crate_max_version[&dep.crate_id];
            let mut incompatible_version = Version(semver::Version {
                major: 0,
                minor: 0,
                patch: 0,
                pre: semver::Prerelease::EMPTY,
                build: semver::BuildMetadata::EMPTY,
            });
            // Produce a synthetic version which is semver incompatible with the
            // highest version currently published.
            if max_version.major > 0 {
                incompatible_version.major = max_version.major + 1;
            } else if max_version.minor > 0 {
                incompatible_version.minor = max_version.minor + 1;
            } else {
                incompatible_version.patch = max_version.patch + 1;
            };
            if dep.req.matches(&incompatible_version) {
                // If the declared dependency requirement claims this crate
                // works with the incompatible future release, we deem the
                // dependency silly and constrain it to remain compatible with
                // the current max published. This affects reqs like `0.*`.
                dep.req.comparators = Slice::new(&[Comparator {
                    op: Op::Caret,
                    major: max_version.major,
                    minor: Some(max_version.minor),
                    patch: Some(max_version.patch),
                    pre: semver::Prerelease::EMPTY,
                }]);
            }
            i += 1;
        }
    }
}

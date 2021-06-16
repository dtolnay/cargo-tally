use cargo_tally::cratemap::CrateMap;
use cargo_tally::DbDump;
use regex::Regex;

pub(crate) fn filter(db_dump: &mut DbDump, crates: &CrateMap, exclude: &[Regex]) {
    if exclude.is_empty() {
        return;
    }
    db_dump.releases.retain(|rel| {
        let crate_name = crates.name(rel.crate_id).unwrap();
        exclude.iter().all(|exclude| !exclude.is_match(crate_name))
    });
}

use crate::cratemap::CrateMap;
use cargo_tally::{DbDump, Query};
use regex::Regex;
use std::collections::{BTreeMap as Map, BTreeSet as Set};

pub(crate) fn exclude(db_dump: &mut DbDump, crates: &CrateMap, exclude: &[Regex]) {
    if exclude.is_empty() {
        return;
    }
    db_dump.releases.retain(|rel| {
        let crate_name = crates.name(rel.crate_id).unwrap();
        exclude.iter().all(|exclude| !exclude.is_match(crate_name))
    });
}

pub(crate) fn disjoin(db_dump: &mut DbDump, queries: &[Query]) {
    let mut version_id_to_crate_id = Map::new();
    for release in &db_dump.releases {
        version_id_to_crate_id.insert(release.id, release.crate_id);
    }

    let mut crate_dependency_graph = Map::new();
    for dep in &db_dump.dependencies {
        if let Some(crate_id) = version_id_to_crate_id.get(&dep.version_id) {
            crate_dependency_graph
                .entry(dep.crate_id)
                .or_insert_with(Set::new)
                .insert(*crate_id);
        }
    }

    let mut visit_crates = Set::new();
    for query in queries {
        for pred in query.predicates {
            visit_crates.insert(pred.crate_id);
        }
    }

    let mut connected_crates = visit_crates.clone();
    loop {
        let mut next = Set::new();
        for crate_id in visit_crates {
            if let Some(reverse_deps) = crate_dependency_graph.get(&crate_id) {
                for connected_crate_id in reverse_deps {
                    if connected_crates.insert(*connected_crate_id) {
                        next.insert(*connected_crate_id);
                    }
                }
            }
        }
        if next.is_empty() {
            break;
        }
        visit_crates = next;
    }

    db_dump.releases.retain(|rel| connected_crates.contains(&rel.crate_id));

    db_dump
        .dependencies
        .retain(|dep| connected_crates.contains(&dep.crate_id));
}

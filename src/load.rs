use crate::cratemap::CrateMap;
use crate::user::User;
use cargo_tally::arena::Slice;
use cargo_tally::dependency::DependencyKind;
use cargo_tally::feature::{CrateFeature, DefaultFeatures, FeatureId, FeatureNames};
use cargo_tally::id::{CrateId, DependencyId, VersionId};
use cargo_tally::timestamp::NaiveDateTime;
use cargo_tally::version::{Version, VersionReq};
use cargo_tally::{DbDump, Dependency, Release};
use db_dump::crate_owners::OwnerId;
use db_dump::Result;
use std::cell::RefCell;
use std::collections::{BTreeMap as Map, BTreeSet as Set};
use std::iter::FromIterator;
use std::mem;
use std::path::Path;

pub(crate) fn load(path: impl AsRef<Path>) -> Result<(DbDump, CrateMap)> {
    let mut crates = CrateMap::new();
    let mut users = Map::new();
    let mut owners = Map::new();
    let mut releases = Vec::new();
    let mut dependencies = Vec::new();
    let mut release_features = Vec::new();
    let feature_names = RefCell::new(FeatureNames::new());

    db_dump::Loader::new()
        .crates(|row| {
            let crate_id = CrateId::from(row.id);
            crates.insert(crate_id, row.name);
        })
        .users(|row| {
            users.insert(User::new(row.gh_login), row.id);
        })
        .crate_owners(|row| {
            if let OwnerId::User(user_id) = row.owner_id {
                owners
                    .entry(user_id)
                    .or_insert_with(Vec::new)
                    .push(CrateId::from(row.crate_id));
            }
        })
        .versions(|row| {
            if row.yanked {
                return;
            }
            let crate_id = CrateId::from(row.crate_id);
            let mut features = Vec::new();
            if !row.features.is_empty() {
                let mut feature_names = feature_names.borrow_mut();
                for (feature, enables) in &row.features {
                    let feature_id = feature_names.id(feature);
                    let enables = enables
                        .iter()
                        .map(|feature| {
                            let crate_id;
                            let mut feature = feature.as_str();
                            if let Some(slash) = feature.find('/') {
                                let crate_name = &feature[..slash];
                                crate_id = feature_names.id(crate_name);
                                feature = &feature[slash + 1..];
                            } else {
                                crate_id = FeatureId::CRATE;
                            }
                            let feature_id = feature_names.id(feature);
                            CrateFeature {
                                crate_id: CrateId(crate_id.0),
                                feature_id,
                            }
                        })
                        .collect::<Vec<_>>();
                    features.push((feature_id, enables));
                }
            }
            releases.push(Release {
                id: VersionId::from(row.id),
                crate_id,
                num: Version(row.num),
                created_at: NaiveDateTime::from(row.created_at),
                features: {
                    release_features.push(features);
                    Slice::EMPTY
                },
            });
        })
        .dependencies(|row| {
            let feature_id = if row.optional {
                FeatureId::TBD
            } else {
                FeatureId::CRATE
            };
            let mut default_features = row.default_features;
            let mut features = Set::new();
            if !row.features.is_empty() {
                let mut feature_names = feature_names.borrow_mut();
                for feature in &row.features {
                    let feature_id = feature_names.id(feature);
                    if feature_id == FeatureId::DEFAULT {
                        default_features = true;
                    } else {
                        features.insert(feature_id);
                    }
                }
            }
            dependencies.push(Dependency {
                id: DependencyId::from(row.id),
                version_id: VersionId::from(row.version_id),
                crate_id: CrateId::from(row.crate_id),
                req: VersionReq::from(row.req),
                feature_id,
                default_features: DefaultFeatures(default_features),
                features: Slice::from_iter(features),
                kind: DependencyKind::from(row.kind),
            });
        })
        .load(path)?;

    let mut feature_names = mem::take(&mut *feature_names.borrow_mut());
    let mut feature_buffer = Vec::new();
    for (release, mut features) in releases.iter_mut().zip(release_features) {
        for (feature, enables) in &mut features {
            // TODO: use retain_mut or drain_filter
            let mut i = 0;
            while let Some(feature) = enables.get_mut(i) {
                let feature_id = FeatureId(feature.crate_id.0);
                feature.crate_id = if feature_id == FeatureId::CRATE {
                    release.crate_id
                } else if let Some(crate_id) = {
                    let name = feature_names.name(feature_id);
                    crates.id(name)
                } {
                    crate_id
                } else {
                    // crates.io's API is lossy :(
                    // https://github.com/rust-lang/crates.io/issues/1539
                    enables.remove(i);
                    continue;
                };
                i += 1;
            }
            feature_buffer.push((*feature, Slice::new(enables)));
        }
        release.features = Slice::new(&feature_buffer);
        feature_buffer.clear();
    }
    for dep in &mut dependencies {
        if dep.feature_id == FeatureId::TBD {
            let crate_name = crates.name(dep.crate_id).unwrap();
            dep.feature_id = feature_names.id(crate_name);
        }
    }

    let db_dump = DbDump {
        releases,
        dependencies,
        features: feature_names,
    };

    crates.users = users;
    crates.owners = owners;

    Ok((db_dump, crates))
}

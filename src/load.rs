use crate::cratemap::CrateMap;
use crate::user::User;
use anyhow::{bail, Result};
use cargo_tally::arena::Slice;
use cargo_tally::dependency::DependencyKind;
use cargo_tally::feature::{
    CrateFeature, DefaultFeatures, FeatureEnables, FeatureId, FeatureNames,
};
use cargo_tally::id::{CrateId, DependencyId, VersionId};
use cargo_tally::timestamp::DateTime;
use cargo_tally::version::{Version, VersionReq};
use cargo_tally::{DbDump, Dependency, Release};
use db_dump::crate_owners::OwnerId;
use std::cell::RefCell;
use std::collections::{BTreeMap as Map, BTreeSet as Set};
use std::mem;
use std::path::Path;

pub(crate) fn load(path: impl AsRef<Path>) -> Result<(DbDump, CrateMap)> {
    let mut crates = CrateMap::new();
    let mut users: Map<User, OwnerId> = Map::new();
    let mut teams: Map<User, OwnerId> = Map::new();
    let mut owners: Map<OwnerId, Vec<CrateId>> = Map::new();
    let mut releases: Vec<Release> = Vec::new();
    let mut dependencies: Vec<Dependency> = Vec::new();
    let mut release_features: Vec<Vec<(FeatureId, Vec<CrateFeature>, Vec<CrateFeature>)>> =
        Vec::new();
    let mut dep_renames: Map<DependencyId, String> = Map::new();
    let mut dep_renames_resolve: Map<(VersionId, FeatureId), CrateId> = Map::new();
    let feature_names = RefCell::new(FeatureNames::new());

    db_dump::Loader::new()
        .crates(|row| {
            let crate_id = CrateId::from(row.id);
            crates.insert(crate_id, row.name);
        })
        .users(|row| {
            users.insert(User::new(row.gh_login), OwnerId::User(row.id));
        })
        .teams(|row| {
            if let Some(team) = row.login.strip_prefix("github:") {
                if team.contains(':') {
                    let team = team.replace(':', "/");
                    teams.insert(User::new(team), OwnerId::Team(row.id));
                }
            }
        })
        .crate_owners(|row| {
            owners
                .entry(row.owner_id)
                .or_insert_with(Vec::new)
                .push(CrateId::from(row.crate_id));
        })
        .versions(|row| {
            if row.yanked {
                return;
            }
            let crate_id = CrateId::from(row.crate_id);
            let mut features = Vec::new();
            if !row.features.is_empty() {
                let mut feature_names = feature_names.borrow_mut();
                for (feature, raw_enables) in &row.features {
                    let feature_id = feature_names.id(feature);
                    let mut enables = Vec::new();
                    let mut weak_enables = Vec::new();
                    for feature in raw_enables {
                        let crate_id;
                        let mut crate_feature_vec = &mut enables;
                        let mut feature = feature.as_str();
                        if let Some(slash) = feature.find('/') {
                            let mut crate_name = &feature[..slash];
                            if let Some(crate_name_weak) = crate_name.strip_suffix('?') {
                                crate_name = crate_name_weak;
                                crate_feature_vec = &mut weak_enables;
                            }
                            crate_id = feature_names.id(crate_name);
                            feature = &feature[slash + 1..];
                        } else {
                            crate_id = FeatureId::CRATE;
                        }
                        let feature_id = feature_names.id(feature);
                        crate_feature_vec.push(CrateFeature {
                            crate_id: CrateId(crate_id.0),
                            feature_id,
                        });
                    }
                    features.push((feature_id, enables, weak_enables));
                }
            }
            releases.push(Release {
                id: VersionId::from(row.id),
                crate_id,
                num: Version(row.num),
                created_at: DateTime::from(row.created_at),
                features: {
                    release_features.push(features);
                    Slice::EMPTY
                },
            });
        })
        .dependencies(|row| {
            let dependency_id = DependencyId::from(row.id);
            let version_id = VersionId::from(row.version_id);
            let crate_id = CrateId::from(row.crate_id);
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
            if let Some(explicit_name) = row.explicit_name {
                let mut feature_names = feature_names.borrow_mut();
                dep_renames_resolve
                    .insert((version_id, feature_names.id(&explicit_name)), crate_id);
                dep_renames.insert(dependency_id, explicit_name);
            }
            dependencies.push(Dependency {
                id: dependency_id,
                version_id,
                crate_id,
                req: VersionReq::from(row.req),
                feature_id,
                default_features: DefaultFeatures(default_features),
                features: Slice::from_iter(features),
                kind: DependencyKind::from(row.kind),
            });
        })
        .load(path)?;

    crate::mend::mend_crates(&mut crates);

    let mut feature_names = mem::take(&mut *feature_names.borrow_mut());
    let mut feature_buffer = Vec::new();
    for (release, mut features) in releases.iter_mut().zip(release_features) {
        for (feature, enables, weak_enables) in &mut features {
            for crate_features in [&mut *enables, &mut *weak_enables] {
                for feature in crate_features {
                    let feature_id = FeatureId(feature.crate_id.0);
                    feature.crate_id = if feature_id == FeatureId::CRATE {
                        release.crate_id
                    } else if let Some(crate_id) =
                        dep_renames_resolve.get(&(release.id, feature_id))
                    {
                        *crate_id
                    } else if let Some(crate_id) = {
                        let name = feature_names.name(feature_id);
                        crates.id(name)
                    } {
                        crate_id
                    } else {
                        bail!(
                            "{} v{} depends on {} which is not found",
                            crates.name(release.crate_id).unwrap(),
                            release.num,
                            feature_names.name(feature_id),
                        );
                    };
                }
            }
            feature_buffer.push(FeatureEnables {
                id: *feature,
                enables: Slice::new(enables),
                weak_enables: Slice::new(weak_enables),
            });
        }
        release.features = Slice::new(&feature_buffer);
        feature_buffer.clear();
    }
    for dep in &mut dependencies {
        if dep.feature_id == FeatureId::TBD {
            dep.feature_id = feature_names.id(match dep_renames.get(&dep.id) {
                Some(explicit_name) => explicit_name,
                None => crates.name(dep.crate_id).unwrap(),
            });
        }
    }

    let mut db_dump = DbDump {
        releases,
        dependencies,
        features: feature_names,
    };

    crates.owners = owners;
    crates.users = users;
    crates.users.extend(teams);

    crate::mend::mend_releases(&mut db_dump, &crates);

    Ok((db_dump, crates))
}

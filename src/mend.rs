use crate::cratemap::CrateMap;
use cargo_tally::arena::Slice;
use cargo_tally::dependency::DependencyKind;
use cargo_tally::feature::{CrateFeature, DefaultFeatures, FeatureEnables, FeatureId};
use cargo_tally::id::{DependencyId, VersionId};
use cargo_tally::{DbDump, Dependency, Release};
use std::collections::BTreeSet as Set;

// Fill back in some deleted crates that cause nontrivial number of dependencies
// downstream to fail to resolve.
pub(crate) fn mend(db_dump: &mut DbDump, crates: &CrateMap) {
    let mut used_version_ids = Set::new();
    let mut used_version_numbers = Set::new();
    for rel in &db_dump.releases {
        used_version_ids.insert(rel.id);
        used_version_numbers.insert((rel.crate_id, rel.num.clone()));
    }

    let mut used_dependency_ids = Set::new();
    for dep in &db_dump.dependencies {
        used_dependency_ids.insert(dep.id);
    }

    let mut next_version_id = VersionId(0);
    let mut next_version_id = || {
        while !used_version_ids.insert(next_version_id) {
            next_version_id.0 += 1;
        }
        next_version_id
    };

    let mut next_dependency_id = DependencyId(0);
    let mut next_dependency_id = || {
        while !used_dependency_ids.insert(next_dependency_id) {
            next_dependency_id.0 += 1;
        }
        next_dependency_id
    };

    let releases = &mut db_dump.releases;
    let mut push_release = |rel: Release| {
        assert!(used_version_numbers.insert((rel.crate_id, rel.num.clone())));
        releases.push(rel);
    };

    {
        let crate_id = crates.id("git-version").unwrap();

        push_release(Release {
            id: next_version_id(),
            crate_id,
            num: version!(0.1.0),
            created_at: datetime!(18 Oct 2017 13:53:11),
            features: Slice::EMPTY,
        });

        push_release(Release {
            id: next_version_id(),
            crate_id,
            num: version!(0.1.1),
            created_at: datetime!(18 Oct 2017 13:55:40),
            features: Slice::EMPTY,
        });

        push_release(Release {
            id: next_version_id(),
            crate_id,
            num: version!(0.1.2),
            created_at: datetime!(18 Oct 2017 13:57:15),
            features: Slice::EMPTY,
        });

        push_release(Release {
            id: next_version_id(),
            crate_id,
            num: version!(0.2.0),
            created_at: datetime!(5 Apr 2018 09:14:16),
            features: Slice::EMPTY,
        });
    }

    {
        let crate_id = crates.id("partial-io").unwrap();

        let features = Slice::new(&[FeatureEnables {
            id: db_dump.features.id("tokio"),
            enables: Slice::new(&[
                CrateFeature {
                    crate_id,
                    feature_id: db_dump.features.id("tokio-io"),
                },
                CrateFeature {
                    crate_id,
                    feature_id: db_dump.features.id("futures"),
                },
            ]),
            weak_enables: Slice::new(&[]),
        }]);

        push_release({
            let release = Release {
                id: next_version_id(),
                crate_id,
                num: version!(0.1.0),
                created_at: datetime!(26 May 2017 02:38:58),
                features,
            };
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("futures").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("futures"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("lazy_static").unwrap(),
                req: version_req!(^0.2),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: db_dump.features.id("quickcheck"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-core").unwrap(),
                req: version_req!(^0.1),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-io").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("tokio-io"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            release
        });

        push_release({
            let release = Release {
                id: next_version_id(),
                crate_id,
                num: version!(0.1.1),
                created_at: datetime!(27 May 2017 00:56:37),
                features,
            };
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("futures").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("futures"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("lazy_static").unwrap(),
                req: version_req!(^0.2),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: db_dump.features.id("quickcheck"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-core").unwrap(),
                req: version_req!(^0.1),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-io").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("tokio-io"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            release
        });

        push_release({
            let release = Release {
                id: next_version_id(),
                crate_id,
                num: version!(0.2.0),
                created_at: datetime!(30 May 2017 21:01:28),
                features,
            };
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("futures").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("futures"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("lazy_static").unwrap(),
                req: version_req!(^0.2),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: db_dump.features.id("quickcheck"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-core").unwrap(),
                req: version_req!(^0.1),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-io").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("tokio-io"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            release
        });

        push_release({
            let release = Release {
                id: next_version_id(),
                crate_id,
                num: version!(0.2.1),
                created_at: datetime!(30 May 2017 21:47:41),
                features,
            };
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("futures").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("futures"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("lazy_static").unwrap(),
                req: version_req!(^0.2),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: db_dump.features.id("quickcheck"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-core").unwrap(),
                req: version_req!(^0.1),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-io").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("tokio-io"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            release
        });

        push_release({
            let release = Release {
                id: next_version_id(),
                crate_id,
                num: version!(0.2.2),
                created_at: datetime!(12 Jun 2017 05:26:52),
                features,
            };
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("futures").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("futures"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("lazy_static").unwrap(),
                req: version_req!(^0.2),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: db_dump.features.id("quickcheck"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-core").unwrap(),
                req: version_req!(^0.1),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-io").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("tokio-io"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            release
        });

        push_release({
            let release = Release {
                id: next_version_id(),
                crate_id,
                num: version!(0.2.3),
                created_at: datetime!(20 Jul 2017 20:01:22),
                features,
            };
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("futures").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("futures"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("lazy_static").unwrap(),
                req: version_req!(^0.2),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: db_dump.features.id("quickcheck"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-core").unwrap(),
                req: version_req!(^0.1),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-io").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("tokio-io"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            release
        });

        push_release({
            let release = Release {
                id: next_version_id(),
                crate_id,
                num: version!(0.2.4),
                created_at: datetime!(19 Aug 2017 23:37:51),
                features,
            };
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("futures").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("futures"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("lazy_static").unwrap(),
                req: version_req!(^0.2),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: db_dump.features.id("quickcheck"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-core").unwrap(),
                req: version_req!(^0.1),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-io").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("tokio-io"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            release
        });

        push_release({
            let release = Release {
                id: next_version_id(),
                crate_id,
                num: version!(0.2.5),
                created_at: datetime!(18 Nov 2017 02:26:25),
                features,
            };
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("futures").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("futures"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("lazy_static").unwrap(),
                req: version_req!(^0.2),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.4),
                feature_id: db_dump.features.id("quickcheck"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-core").unwrap(),
                req: version_req!(^0.1),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-io").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("tokio-io"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            release
        });

        push_release({
            let release = Release {
                id: next_version_id(),
                crate_id,
                num: version!(0.3.0),
                created_at: datetime!(12 Jan 2018 22:15:15),
                features,
            };
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("futures").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("futures"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("lazy_static").unwrap(),
                req: version_req!(^1.0),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.6),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("quickcheck").unwrap(),
                req: version_req!(^0.6),
                feature_id: db_dump.features.id("quickcheck"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-core").unwrap(),
                req: version_req!(^0.1),
                feature_id: FeatureId::CRATE,
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Dev,
            });
            db_dump.dependencies.push(Dependency {
                id: next_dependency_id(),
                version_id: release.id,
                crate_id: crates.id("tokio-io").unwrap(),
                req: version_req!(^0.1),
                feature_id: db_dump.features.id("tokio-io"),
                default_features: DefaultFeatures(true),
                features: Slice::EMPTY,
                kind: DependencyKind::Normal,
            });
            release
        });
    }
}

#![allow(non_camel_case_types)]
#![allow(
    clippy::borrow_as_ptr,
    clippy::borrowed_box,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_ptr_alignment,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::iter_not_returning_iterator, // https://github.com/rust-lang/rust-clippy/issues/8285
    clippy::mismatching_type_param_order, // https://github.com/rust-lang/rust-clippy/issues/8962
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::needless_pass_by_value,
    clippy::option_if_let_else,
    clippy::ptr_as_ptr,
    clippy::significant_drop_in_scrutinee,
    clippy::too_many_lines,
    clippy::uninlined_format_args,
    clippy::unseparated_literal_suffix
)]

#[macro_use]
mod stream;

pub mod arena;
pub(crate) mod collect;
mod communication;
pub mod dependency;
pub mod feature;
pub(crate) mod hint;
pub mod id;
mod impls;
pub mod matrix;
pub(crate) mod max;
pub(crate) mod present;
pub mod timestamp;
pub mod version;

use crate::arena::Slice;
use crate::collect::{Collect, Emitter, ResultCollection};
use crate::dependency::DependencyKind;
use crate::feature::{
    DefaultFeatures, FeatureEnables, FeatureId, FeatureIter, FeatureNames, VersionFeature,
};
use crate::hint::TypeHint;
use crate::id::{CrateId, DependencyId, QueryId, VersionId};
use crate::matrix::Matrix;
use crate::max::MaxByKey;
use crate::present::Present;
use crate::timestamp::{DateTime, Duration};
use crate::version::{Version, VersionReq};
use atomic_take::AtomicTake;
use differential_dataflow::input::InputSession;
use differential_dataflow::operators::arrange::{ArrangeByKey, ArrangeBySelf};
use differential_dataflow::operators::iterate::Variable;
use differential_dataflow::operators::{Consolidate, Join, JoinCore, Threshold};
use std::env;
use std::iter::once;
use std::net::TcpStream;
use std::ops::Deref;
use timely::communication::allocator::Process;
use timely::dataflow::operators::capture::EventWriter;
use timely::dataflow::scopes::Child;
use timely::dataflow::Scope;
use timely::logging::{BatchLogger, TimelyEvent};
use timely::order::Product;
use timely::progress::Timestamp;
use timely::worker::{Config as WorkerConfig, Worker};

#[derive(Default)]
pub struct DbDump {
    pub releases: Vec<Release>,
    pub dependencies: Vec<Dependency>,
    pub features: FeatureNames,
}

#[derive(Clone, Debug)]
pub struct Release {
    pub id: VersionId,
    pub crate_id: CrateId,
    pub num: Version,
    pub created_at: DateTime,
    pub features: Slice<FeatureEnables>,
}

#[derive(Copy, Clone, Debug)]
pub struct Dependency {
    pub id: DependencyId,
    pub version_id: VersionId,
    pub crate_id: CrateId,
    pub req: VersionReq,
    pub feature_id: FeatureId,
    pub default_features: DefaultFeatures,
    pub features: Slice<FeatureId>,
    pub kind: DependencyKind,
}

#[derive(Copy, Clone, Debug)]
pub struct Query {
    pub id: QueryId,
    pub predicates: Slice<Predicate>,
}

#[derive(Copy, Clone, Debug)]
pub struct Predicate {
    pub crate_id: CrateId,
    pub req: Option<VersionReq>,
}

#[derive(Default)]
struct Input {
    db_dump: DbDump,
    queries: Vec<Query>,
}

pub fn run(db_dump: DbDump, jobs: usize, transitive: bool, queries: &[Query]) -> Matrix {
    let num_queries = queries.len();
    let queries = queries.to_owned();
    let input = AtomicTake::new(Input { db_dump, queries });
    let collection = ResultCollection::<(QueryId, DateTime, isize)>::new();
    let results = collection.emitter();

    let allocators = Process::new_vector(jobs);
    let other = Box::new(());
    timely::communication::initialize_from(allocators, other, move |allocator| {
        let mut worker = Worker::new(WorkerConfig::default(), allocator);
        set_timely_worker_log(&worker);

        let mut queries = InputSession::<DateTime, Query, Present>::new();
        let mut releases = InputSession::<DateTime, Release, Present>::new();
        let mut dependencies = InputSession::<DateTime, Dependency, Present>::new();

        worker.dataflow(|scope| {
            dataflow(
                scope,
                &mut queries,
                &mut releases,
                &mut dependencies,
                transitive,
                &results,
            );
        });

        let input = input.take().unwrap_or_default();

        for query in input.queries {
            queries.update(query, Present);
        }
        queries.close();

        for dep in input.db_dump.dependencies {
            dependencies.update(dep, Present);
        }
        dependencies.close();

        for rel in input.db_dump.releases {
            releases.advance_to(rel.created_at);
            releases.update(rel, Present);
        }
        releases.close();

        while worker.step_or_park(None) {}
    })
    .unwrap();

    let mut time = DateTime::minimum();
    let mut values = vec![0u32; num_queries];
    let mut matrix = Matrix::new(num_queries);
    collection.sort();
    for (i, (query_id, timestamp, diff)) in collection.into_iter().enumerate() {
        if timestamp > time {
            if i > 0 {
                matrix.push(time, values.clone());
            }
            time = timestamp;
        }
        let cell = &mut values[query_id.0 as usize];
        if diff > 0 {
            *cell += diff as u32;
        } else {
            *cell = cell.checked_sub(-diff as u32).expect("value went negative");
        }
    }
    if match matrix.iter().next_back() {
        Some((_timestamp, last)) => values != **last,
        None => values.iter().any(|&n| n != 0),
    } {
        matrix.push(time, values);
    }
    matrix
}

fn set_timely_worker_log(worker: &Worker<Process>) {
    let Some(addr) = env::var_os("TIMELY_WORKER_LOG_ADDR") else { return };

    let stream = match TcpStream::connect(addr.to_str().unwrap()) {
        Ok(stream) => stream,
        Err(err) => panic!("Could not connect logging stream to {addr:?}: {err}"),
    };

    worker.log_register().insert::<TimelyEvent, _>("timely", {
        let writer = EventWriter::new(stream);
        let mut logger = BatchLogger::new(writer);
        move |time, data| logger.publish_batch(time, data)
    });
}

fn dataflow(
    scope: &mut Child<Worker<Process>, DateTime>,
    queries: &mut InputSession<DateTime, Query, Present>,
    releases: &mut InputSession<DateTime, Release, Present>,
    dependencies: &mut InputSession<DateTime, Dependency, Present>,
    transitive: bool,
    results: &Emitter<(QueryId, DateTime, isize)>,
) {
    type queries<'a> = stream![Query; Present];
    let queries: queries = queries.to_collection(scope);

    type releases<'a> = stream![Release; Present];
    let releases: releases = releases.to_collection(scope);

    type dependencies<'a> = stream![Dependency; Present];
    let dependencies: dependencies = dependencies.to_collection(scope);

    // the version ids and version numbers that exist of each crate
    type releases_by_crate_id<'a> = stream![CrateId => (VersionId, Version); Present];
    let releases_by_crate_id: releases_by_crate_id =
        releases.map(|rel| (rel.crate_id, (rel.id, rel.num)));
    let releases_by_crate_id = releases_by_crate_id.arrange_by_key();

    // for each dependency spec, what release does it refer to currently?
    type resolved<'a> = stream![(CrateId, VersionReq) => VersionId; isize];
    let resolved: resolved = dependencies
        .map(|dep| (dep.crate_id, dep.req))
        .KV::<CrateId, VersionReq>()
        .join_core(
            &releases_by_crate_id,
            |crate_id, req, (version_id, version)| {
                req.matches(version)
                    .then(|| ((*crate_id, *req), (version.clone(), *version_id)))
            },
        )
        .KV::<(CrateId, VersionReq), (Version, VersionId)>()
        .max_by_key()
        .KV::<(CrateId, VersionReq), (Version, VersionId)>()
        .map(|((crate_id, req), (_version, version_id))| ((crate_id, req), version_id));
    let resolved = resolved.arrange_by_key();

    // full dependency graph across all versions of all crates
    type dependency_edges<'a> = stream![VersionId => VersionId; isize];
    let direct_dependency_edges: dependency_edges = dependencies
        .map(|dep| ((dep.crate_id, dep.req), dep.version_id))
        .KV::<(CrateId, VersionReq), VersionId>()
        .join_core(
            &resolved,
            |(_crate_id, _req), from_version_id, to_version_id| {
                once((*from_version_id, *to_version_id))
            },
        );

    // releases that are the most recent of their crate
    type most_recent_crate_version<'a> = stream![VersionId; isize];
    let most_recent_crate_version: most_recent_crate_version = releases
        .map(|rel| {
            (
                rel.crate_id,
                (rel.num.pre.is_empty(), rel.created_at, rel.id),
            )
        })
        .KV::<CrateId, (bool, DateTime, VersionId)>()
        .max_by_key()
        .KV::<CrateId, (bool, DateTime, VersionId)>()
        .map(|(_crate_id, (_not_prerelease, _created_at, version_id))| version_id);
    let most_recent_crate_version = most_recent_crate_version.arrange_by_self();

    // releases that satisfy the predicate of each query
    type match_releases<'a> = stream![VersionId => QueryId; Present];
    let match_releases: match_releases = queries
        .flat_map(|query| {
            query
                .predicates
                .iter()
                .map(move |pred| (pred.crate_id, (query.id, pred.req)))
        })
        .KV::<CrateId, (QueryId, Option<VersionReq>)>()
        .join_core(
            &releases_by_crate_id,
            |_crate_id, (query_id, version_req), (version_id, version)| {
                let matches = match version_req {
                    None => true,
                    Some(req) => req.matches(version),
                };
                matches.then_some((*version_id, *query_id))
            },
        );

    // releases that contribute into the result of each query
    type query_results<'a> = stream![VersionId => QueryId; isize];
    let mut query_results: query_results = direct_dependency_edges
        .join_core(&most_recent_crate_version, |edge_from, edge_to, ()| {
            once((*edge_to, *edge_from))
        })
        .KV::<VersionId, VersionId>()
        .join_map(&match_releases, |_edge_to, edge_from, query_id| {
            (*edge_from, *query_id)
        });

    if transitive {
        type dependency_edges<'a> = stream![VersionFeature => VersionFeature; isize];

        // dependency edges arising from an entry under [dependencies]
        let dep_dependency_edges: dependency_edges = dependencies
            .flat_map(|dep| match dep.kind {
                DependencyKind::Normal | DependencyKind::Build => Some((
                    (dep.crate_id, dep.req),
                    (
                        dep.version_id,
                        dep.feature_id,
                        dep.default_features,
                        dep.features,
                    ),
                )),
                DependencyKind::Dev => None,
            })
            .KV::<(CrateId, VersionReq), (VersionId, FeatureId, DefaultFeatures, Slice<FeatureId>)>(
            )
            .join_core(
                &resolved,
                |(_crate_id, _req),
                 (version_id, feature_id, default_features, features),
                 resolved_version_id| {
                    let edge_from = VersionFeature {
                        version_id: *version_id,
                        feature_id: *feature_id,
                    };
                    let resolved_version_id = *resolved_version_id;
                    FeatureIter::new(*default_features, *features).map(move |feature_id| {
                        let edge_to = VersionFeature {
                            version_id: resolved_version_id,
                            feature_id,
                        };
                        (edge_from, edge_to)
                    })
                },
            );

        // dependency edges from crate feature enabling other feature of same crate
        let feature_intracrate_edges: dependency_edges = releases.explode(|rel| {
            let version_id = rel.id;
            let crate_id = rel.crate_id;
            rel.features
                .iter()
                .flat_map(move |feature| {
                    let edge_from = VersionFeature {
                        version_id,
                        feature_id: feature.id,
                    };
                    feature
                        .enables
                        .into_iter()
                        .filter_map(move |crate_feature| {
                            if crate_feature.crate_id == crate_id {
                                let edge_to = VersionFeature {
                                    version_id,
                                    feature_id: crate_feature.feature_id,
                                };
                                Some((edge_from, edge_to))
                            } else {
                                None
                            }
                        })
                        .chain({
                            if feature.id == FeatureId::DEFAULT {
                                None
                            } else {
                                let edge_to = VersionFeature {
                                    version_id,
                                    feature_id: FeatureId::CRATE,
                                };
                                Some((edge_from, edge_to))
                            }
                        })
                })
                .chain({
                    let edge_from = VersionFeature {
                        version_id,
                        feature_id: FeatureId::DEFAULT,
                    };
                    let edge_to = VersionFeature {
                        version_id,
                        feature_id: FeatureId::CRATE,
                    };
                    once((edge_from, edge_to))
                })
                .map(|(edge_from, edge_to)| ((edge_from, edge_to), 1))
        });

        // dependency edges from crate feature enabling feature of other crate
        let feature_dependency_edges: dependency_edges = releases
            .flat_map(|rel| {
                let version_id = rel.id;
                let crate_id = rel.crate_id;
                rel.features.into_iter().flat_map(move |feature| {
                    // TODO: also handle `weak_enables`
                    // https://github.com/dtolnay/cargo-tally/issues/56
                    feature
                        .enables
                        .into_iter()
                        .filter_map(move |crate_feature| {
                            if crate_feature.crate_id == crate_id {
                                None
                            } else {
                                Some((
                                    (version_id, crate_feature.crate_id),
                                    (feature.id, crate_feature.feature_id),
                                ))
                            }
                        })
                })
            })
            .KV::<(VersionId, CrateId), (FeatureId, FeatureId)>()
            .join_map(
                &dependencies
                    .map(|dep| ((dep.version_id, dep.crate_id), dep.req))
                    .KV::<(VersionId, CrateId), VersionReq>(),
                |(version_id, crate_id), (from_feature, to_feature), req| {
                    ((*crate_id, *req), (*version_id, *from_feature, *to_feature))
                },
            )
            .KV::<(CrateId, VersionReq), (VersionId, FeatureId, FeatureId)>()
            .join_core(
                &resolved,
                |(_crate_id, _req),
                 (from_version_id, from_feature_id, to_feature_id),
                 to_version_id| {
                    let edge_from = VersionFeature {
                        version_id: *from_version_id,
                        feature_id: *from_feature_id,
                    };
                    let edge_to = VersionFeature {
                        version_id: *to_version_id,
                        feature_id: *to_feature_id,
                    };
                    Some((edge_from, edge_to))
                },
            );

        // full dependency graph across all versions of all crates
        let incoming_transitive_dependency_edges = dep_dependency_edges
            .concat(&feature_intracrate_edges)
            .concat(&feature_dependency_edges)
            .KV::<VersionFeature, VersionFeature>()
            .map_in_place(|edge| {
                let (edge_from, edge_to) = *edge;
                *edge = (edge_to, edge_from);
            })
            .KV::<VersionFeature, VersionFeature>()
            .arrange_by_key();

        // fixed point of transitive dependencies graph
        type addend_transitive_releases<'a> = stream![VersionId => QueryId; isize];
        let addend_transitive_releases: addend_transitive_releases = scope
            .iterative::<u16, _, _>(|nested| {
                let match_releases = match_releases
                    .KV::<VersionId, QueryId>()
                    .explode(|(version_id, query_id)| {
                        let version_feature = VersionFeature {
                            version_id,
                            feature_id: FeatureId::CRATE,
                        };
                        once(((version_feature, query_id), 1))
                    })
                    .KV::<VersionFeature, QueryId>()
                    .enter(nested);
                let summary = Product::new(Duration::default(), 1);
                let variable = Variable::new_from(match_releases, summary);
                let result = variable
                    .deref()
                    .KV::<VersionFeature, QueryId>()
                    .join_core(
                        &incoming_transitive_dependency_edges.enter(nested),
                        |_edge_to, query_id, edge_from| Some((*edge_from, *query_id)),
                    )
                    .KV::<VersionFeature, QueryId>()
                    .concat(&variable)
                    .KV::<VersionFeature, QueryId>()
                    .distinct();
                variable.set(&result).leave()
            })
            .KV::<VersionFeature, QueryId>()
            .map(|(version_feature, query_id)| (version_feature.version_id, query_id));

        query_results = addend_transitive_releases
            .join_core(&most_recent_crate_version, |version_id, query_id, ()| {
                Some((*version_id, *query_id))
            })
            .KV::<VersionId, QueryId>()
            .concat(&query_results);
    }

    query_results
        .distinct()
        .map(|(_version_id, query_id)| query_id)
        .consolidate()
        .collect_into(results);
}

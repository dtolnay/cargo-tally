// use cargo_tally::{Crate, Feature, Dependency, DependencyKind};
// use fnv::FnvHashMap as Map;
// use semver::{Version, VersionReq};
// use serde::{Deserialize, Serialize};

// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct CrateMeta {
//     name: String,
//     version: VersionReq,
//     // TODO do i need this info??
//     features: Vec<String>,
//     kind: DependencyKind,
// }

// impl CrateMeta {
//     fn new(dep: &Dependency) -> Self {
//         // TODO try not to clone everything TransitiveCrateDeps too !!!
//         Self {
//             name: dep.name.to_string(),
//             version: dep.req.clone(),
//             features: dep.features.clone(),
//             kind: dep.kind.clone(),
//         }
//     }
// }

// #[derive(Clone, Debug, Serialize, Deserialize)]
// pub struct TranitiveCrateDeps {
//     pub name: String,
//     pub version: Version,
//     pub features: Map<String, Vec<Feature>>,
//     /// Crates that depend on this crate
//     pub depended_on: Vec<CrateMeta>,
//     // or we could just use the count
//     // count: usize,
// }

// impl TranitiveCrateDeps {
//     fn collect_deps(crates: &[Crate], search_crate: &Crate) -> Vec<CrateMeta> {
//         let mut res = Vec::new();
//         for krate in crates {
//             if let Some(dep) = krate.dependencies
//                 .iter()
//                 .find(|dep| {
//                     // collect all crates that depend on `search_crate`, non optional and non dev dep
//                     dep.name == search_crate.name && !dep.optional && dep.kind != DependencyKind::Dev
//                 }) {
//                     res.push(CrateMeta::new(dep));
//                 }
//         }
//         res
//     }
//     pub(crate) fn calc_dependencies(crates: &[Crate], krate: &Crate) -> Self {
//         let depended_on = Self::collect_deps(crates, krate);
//         Self {
//             name: krate.name.to_owned(),
//             version: krate.version.clone(),
//             features: krate.features.clone(),
//             depended_on,
//         }
//     }
//     // ? use tally's Resolve and Universe to compute?
//     pub fn calculate_trans_deps(&self, crates: &[Crate]) {
//         // calculate from Vec<Crate>
//     }
// }

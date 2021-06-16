use crate::arena::Slice;
use crate::id::{CrateId, VersionId};
use std::collections::BTreeMap as Map;
use std::convert::TryFrom;

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct FeatureId(pub u32);

impl FeatureId {
    pub const CRATE: Self = FeatureId(0);
    pub const DEFAULT: Self = FeatureId(1);
    pub const TBD: Self = FeatureId(!0);
}

#[derive(Copy, Clone, Debug)]
pub struct CrateFeature {
    pub crate_id: CrateId,
    pub feature_id: FeatureId,
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
pub struct VersionFeature {
    pub version_id: VersionId,
    pub feature_id: FeatureId,
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct DefaultFeatures(pub bool);

pub struct FeatureNames {
    names: Vec<String>,
    map: Map<String, FeatureId>,
}

impl FeatureNames {
    pub fn new() -> Self {
        let mut feature_names = FeatureNames {
            names: Vec::new(),
            map: Map::new(),
        };
        assert_eq!(feature_names.id(""), FeatureId::CRATE);
        assert_eq!(feature_names.id("default"), FeatureId::DEFAULT);
        feature_names
    }

    pub fn id(&mut self, name: &str) -> FeatureId {
        if let Some(id) = self.map.get(name) {
            *id
        } else {
            let new_id = FeatureId(u32::try_from(self.names.len()).unwrap());
            self.names.push(name.to_owned());
            self.map.insert(name.to_owned(), new_id);
            new_id
        }
    }

    pub fn name(&self, id: FeatureId) -> &str {
        &self.names[id.0 as usize]
    }
}

impl Default for FeatureNames {
    fn default() -> Self {
        FeatureNames::new()
    }
}

pub struct FeatureIter {
    krate: bool,
    default: bool,
    other: <Slice<FeatureId> as IntoIterator>::IntoIter,
}

impl FeatureIter {
    pub fn new(default_features: DefaultFeatures, features: Slice<FeatureId>) -> Self {
        FeatureIter {
            krate: !default_features.0 && features.is_empty(),
            default: default_features.0,
            other: features.into_iter(),
        }
    }
}

impl Iterator for FeatureIter {
    type Item = FeatureId;

    fn next(&mut self) -> Option<Self::Item> {
        if self.krate {
            self.krate = false;
            Some(FeatureId::CRATE)
        } else if self.default {
            self.default = false;
            Some(FeatureId::DEFAULT)
        } else {
            self.other.next()
        }
    }
}

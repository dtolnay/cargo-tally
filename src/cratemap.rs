use crate::id::CrateId;
use std::borrow::Borrow;
use std::collections::BTreeMap as Map;

#[derive(Default)]
pub struct CrateMap {
    names: Map<CrateId, String>,
    ids: Map<String, CrateId>,
    normalized: Map<String, CrateId>,
}

impl CrateMap {
    pub fn new() -> Self {
        CrateMap::default()
    }

    pub fn insert(&mut self, id: CrateId, name: String) {
        assert!(!self.ids.contains_key(&name));
        assert!(!self.names.contains_key(&id));
        self.ids.insert(name.clone(), id);
        self.normalized.insert(normalize(&name), id);
        self.names.insert(id, name);
    }

    pub fn name(&self, id: CrateId) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    pub fn id<Q>(&self, name: &Q) -> Option<CrateId>
    where
        String: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.ids.get(name).copied()
    }

    pub fn id_normalized(&self, fuzzy_name: &str) -> Option<CrateId> {
        let name = normalize(fuzzy_name);
        self.normalized.get(&name).copied()
    }
}

fn normalize(name: &str) -> String {
    name.replace('_', "-")
}

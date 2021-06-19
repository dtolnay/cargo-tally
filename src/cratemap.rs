use crate::cratename::{CrateName, CrateNameQuery};
use crate::user::User;
use cargo_tally::id::CrateId;
use db_dump::users::UserId;
use ref_cast::RefCast;
use std::collections::BTreeMap as Map;

#[derive(Default)]
pub struct CrateMap {
    names: Map<CrateId, String>,
    ids: Map<CrateName, CrateId>,
    pub(crate) users: Map<User, UserId>,
    pub(crate) owners: Map<UserId, Vec<CrateId>>,
}

impl CrateMap {
    pub fn new() -> Self {
        CrateMap::default()
    }

    pub fn insert(&mut self, id: CrateId, name: String) {
        assert!(!self.ids.contains_key(CrateNameQuery::ref_cast(&name)));
        assert!(!self.names.contains_key(&id));
        self.ids.insert(CrateName::new(name.clone()), id);
        self.names.insert(id, name);
    }

    pub fn name(&self, id: CrateId) -> Option<&str> {
        self.names.get(&id).map(String::as_str)
    }

    pub fn id(&self, name: &str) -> Option<CrateId> {
        self.ids.get(CrateNameQuery::ref_cast(name)).copied()
    }
}

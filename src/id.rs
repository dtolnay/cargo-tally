#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct QueryId(pub u8);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct CrateId(pub u32);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct VersionId(pub u32);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[repr(transparent)]
pub struct DependencyId(pub u32);

impl From<db_dump::crates::CrateId> for CrateId {
    fn from(id: db_dump::crates::CrateId) -> Self {
        CrateId(id.0)
    }
}

impl From<db_dump::versions::VersionId> for VersionId {
    fn from(id: db_dump::versions::VersionId) -> Self {
        VersionId(id.0)
    }
}

impl From<u32> for DependencyId {
    fn from(id: u32) -> Self {
        DependencyId(id)
    }
}

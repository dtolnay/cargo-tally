#[derive(Copy, Clone, Debug)]
pub enum DependencyKind {
    Normal,
    Build,
    Dev,
}

impl From<db_dump::dependencies::DependencyKind> for DependencyKind {
    fn from(dependency_kind: db_dump::dependencies::DependencyKind) -> Self {
        match dependency_kind {
            db_dump::dependencies::DependencyKind::Normal => DependencyKind::Normal,
            db_dump::dependencies::DependencyKind::Build => DependencyKind::Build,
            db_dump::dependencies::DependencyKind::Dev => DependencyKind::Dev,
        }
    }
}

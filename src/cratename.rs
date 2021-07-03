use ref_cast::RefCast;
use std::borrow::Borrow;
use std::cmp::Ordering;

pub const MAX_NAME_LENGTH: usize = 64;

// Mirrored from https://github.com/rust-lang/crates.io/blob/54a3f10794db7f57e3602426389c369290a8a3d5/src/models/krate.rs
pub fn valid(name: &str) -> bool {
    let under_max_length = name.chars().take(MAX_NAME_LENGTH + 1).count() <= MAX_NAME_LENGTH;
    valid_ident(name) && under_max_length
}

fn valid_ident(name: &str) -> bool {
    valid_feature_prefix(name) && name.chars().next().map_or(false, char::is_alphabetic)
}

fn valid_feature_prefix(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

pub(crate) struct CrateName(String);

impl CrateName {
    pub(crate) fn new(string: String) -> Self {
        CrateName(string)
    }
}

impl Ord for CrateName {
    fn cmp(&self, rhs: &Self) -> Ordering {
        CrateNameQuery::ref_cast(&self.0).cmp(CrateNameQuery::ref_cast(&rhs.0))
    }
}

impl PartialOrd for CrateName {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Eq for CrateName {}

impl PartialEq for CrateName {
    fn eq(&self, rhs: &Self) -> bool {
        CrateNameQuery::ref_cast(&self.0).eq(CrateNameQuery::ref_cast(&rhs.0))
    }
}

#[derive(RefCast)]
#[repr(transparent)]
pub(crate) struct CrateNameQuery(str);

impl Borrow<CrateNameQuery> for CrateName {
    fn borrow(&self) -> &CrateNameQuery {
        CrateNameQuery::ref_cast(&self.0)
    }
}

impl Ord for CrateNameQuery {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.0
            .bytes()
            .map(SeparatorAgnosticByte)
            .cmp(rhs.0.bytes().map(SeparatorAgnosticByte))
    }
}

impl PartialOrd for CrateNameQuery {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Eq for CrateNameQuery {}

impl PartialEq for CrateNameQuery {
    fn eq(&self, rhs: &Self) -> bool {
        self.0
            .bytes()
            .map(SeparatorAgnosticByte)
            .eq(rhs.0.bytes().map(SeparatorAgnosticByte))
    }
}

struct SeparatorAgnosticByte(u8);

impl Ord for SeparatorAgnosticByte {
    fn cmp(&self, rhs: &Self) -> Ordering {
        let lhs = if self.0 == b'_' { b'-' } else { self.0 };
        let rhs = if rhs.0 == b'_' { b'-' } else { rhs.0 };
        lhs.cmp(&rhs)
    }
}

impl PartialOrd for SeparatorAgnosticByte {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Eq for SeparatorAgnosticByte {}

impl PartialEq for SeparatorAgnosticByte {
    fn eq(&self, rhs: &Self) -> bool {
        self.cmp(rhs) == Ordering::Equal
    }
}

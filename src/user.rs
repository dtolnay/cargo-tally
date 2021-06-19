use ref_cast::RefCast;
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::fmt::{self, Display};

pub(crate) fn valid(name: &str) -> bool {
    name.chars().all(|ch| {
        (ch >= '0' && ch <= '9')
            || (ch >= 'A' && ch <= 'Z')
            || (ch >= 'a' && ch <= 'z')
            || ch == '-'
    }) && !name.contains("--")
        && !name.starts_with('-')
        && !name.ends_with('-')
        && !name.is_empty()
        && name.len() <= 39
}

pub(crate) struct User(String);

impl User {
    pub(crate) fn new(string: String) -> Self {
        User(string)
    }
}

impl Ord for User {
    fn cmp(&self, rhs: &Self) -> Ordering {
        UserQuery::ref_cast(&self.0).cmp(UserQuery::ref_cast(&rhs.0))
    }
}

impl PartialOrd for User {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Eq for User {}

impl PartialEq for User {
    fn eq(&self, rhs: &Self) -> bool {
        UserQuery::ref_cast(&self.0).eq(UserQuery::ref_cast(&rhs.0))
    }
}

impl Display for User {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

#[derive(RefCast)]
#[repr(transparent)]
pub(crate) struct UserQuery(str);

impl UserQuery {
    pub(crate) fn is_team(&self) -> bool {
        self.0.contains('/')
    }
}

impl Borrow<UserQuery> for User {
    fn borrow(&self) -> &UserQuery {
        UserQuery::ref_cast(&self.0)
    }
}

impl Ord for UserQuery {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.0
            .bytes()
            .map(CaseAgnosticByte)
            .cmp(rhs.0.bytes().map(CaseAgnosticByte))
    }
}

impl PartialOrd for UserQuery {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Eq for UserQuery {}

impl PartialEq for UserQuery {
    fn eq(&self, rhs: &Self) -> bool {
        self.0
            .bytes()
            .map(CaseAgnosticByte)
            .eq(rhs.0.bytes().map(CaseAgnosticByte))
    }
}

impl Display for UserQuery {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

struct CaseAgnosticByte(u8);

impl Ord for CaseAgnosticByte {
    fn cmp(&self, rhs: &Self) -> Ordering {
        self.0.to_ascii_lowercase().cmp(&rhs.0.to_ascii_lowercase())
    }
}

impl PartialOrd for CaseAgnosticByte {
    fn partial_cmp(&self, rhs: &Self) -> Option<Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Eq for CaseAgnosticByte {}

impl PartialEq for CaseAgnosticByte {
    fn eq(&self, rhs: &Self) -> bool {
        self.cmp(rhs) == Ordering::Equal
    }
}

use crate::arena::Slice;
use semver::Op as SemverOp;
use std::convert::TryFrom;
use std::fmt::{self, Debug, Display};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct VersionReq {
    pub comparators: Slice<Comparator>,
}

impl VersionReq {
    pub const ANY: Self = VersionReq {
        comparators: Slice::EMPTY,
    };
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Comparator {
    pub op: Op,
    pub major: u64,
    pub minor: Option<u64>,
    pub patch: Option<u64>,
}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub enum Op {
    Exact,
    Greater,
    GreaterEq,
    Less,
    LessEq,
    Tilde,
    Caret,
}

impl VersionReq {
    pub fn matches(&self, version: &Version) -> bool {
        self.comparators
            .iter()
            .all(|comparator| comparator.matches(version))
    }
}

impl Comparator {
    fn matches(&self, version: &Version) -> bool {
        match self.op {
            Op::Exact => self.matches_exact(version),
            Op::Greater => self.matches_greater(version),
            Op::GreaterEq => self.matches_exact(version) || self.matches_greater(version),
            Op::Less => !self.matches_exact(version) && !self.matches_greater(version),
            Op::LessEq => !self.matches_greater(version),
            Op::Tilde => self.matches_tilde(version),
            Op::Caret => self.matches_compatible(version),
        }
    }

    fn matches_exact(&self, version: &Version) -> bool {
        version.major == self.major
            && self.minor.map_or(true, |minor| version.minor == minor)
            && self.patch.map_or(true, |patch| version.patch == patch)
    }

    fn matches_greater(&self, version: &Version) -> bool {
        if version.major != self.major {
            return version.major > self.major;
        }

        match self.minor {
            None => return false,
            Some(minor) => {
                if version.minor != minor {
                    return version.minor > minor;
                }
            }
        }

        self.patch.map_or(false, |patch| version.patch > patch)
    }

    fn matches_tilde(&self, version: &Version) -> bool {
        version.major == self.major
            && self.minor.map_or(true, |minor| version.minor == minor)
            && self.patch.map_or(true, |patch| version.patch >= patch)
    }

    fn matches_compatible(&self, version: &Version) -> bool {
        if version.major != self.major {
            return false;
        }

        let minor = match self.minor {
            None => return true,
            Some(minor) => minor,
        };

        match self.patch {
            None => {
                if self.major > 0 {
                    version.minor >= minor
                } else {
                    version.minor == minor
                }
            }
            Some(patch) => {
                if self.major > 0 {
                    version.minor > minor || (version.minor == minor && version.patch >= patch)
                } else if minor > 0 {
                    version.minor == minor && version.patch >= patch
                } else {
                    version.minor == minor && version.patch == patch
                }
            }
        }
    }
}

impl Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Debug for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Version({})", self)
    }
}

impl Display for VersionReq {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if self.comparators.is_empty() {
            return formatter.write_str("*");
        }
        for (i, comparator) in self.comparators.iter().enumerate() {
            if i > 0 {
                formatter.write_str(", ")?;
            }
            formatter.write_str(match comparator.op {
                Op::Exact => "=",
                Op::Greater => ">",
                Op::GreaterEq => ">=",
                Op::Less => "<",
                Op::LessEq => "<=",
                Op::Tilde => "~",
                Op::Caret => "^",
            })?;
            Display::fmt(&comparator.major, formatter)?;
            if let Some(minor) = comparator.minor {
                write!(formatter, ".{}", minor)?;
            }
            if let Some(patch) = comparator.patch {
                write!(formatter, ".{}", patch)?;
            }
        }
        Ok(())
    }
}

impl Debug for VersionReq {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "VersionReq({})", self)
    }
}

pub struct UnsupportedPrerelease;

impl TryFrom<semver::VersionReq> for VersionReq {
    type Error = UnsupportedPrerelease;

    fn try_from(req: semver::VersionReq) -> Result<Self, Self::Error> {
        let mut comparators = Vec::new();

        for comparator in req.comparators {
            if !comparator.pre.is_empty() {
                return Err(UnsupportedPrerelease);
            }
            assert!(comparator.minor.is_some() || comparator.patch.is_none());
            let op = match comparator.op {
                SemverOp::Exact | SemverOp::Wildcard => Op::Exact,
                SemverOp::Greater => Op::Greater,
                SemverOp::GreaterEq => Op::GreaterEq,
                SemverOp::Less => Op::Less,
                SemverOp::LessEq => Op::LessEq,
                SemverOp::Tilde => Op::Tilde,
                SemverOp::Caret => Op::Caret,
                _ => unimplemented!(),
            };
            comparators.push(Comparator {
                op,
                major: comparator.major,
                minor: comparator.minor,
                patch: comparator.patch,
            });
        }

        let comparators = Slice::new(&comparators);
        Ok(VersionReq { comparators })
    }
}

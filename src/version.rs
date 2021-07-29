use crate::arena::Slice;
use semver::{Comparator, Op};
use std::cmp::Ordering;
use std::fmt::{self, Debug, Display};
use std::ops::{Deref, DerefMut};
use std::str::FromStr;

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Version(pub semver::Version);

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct VersionReq {
    pub comparators: Slice<Comparator>,
}

impl VersionReq {
    pub fn matches(&self, version: &Version) -> bool {
        matches_req(self.comparators, version)
    }
}

impl Deref for Version {
    type Target = semver::Version;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Version {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

impl Debug for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "Version({})", self)
    }
}

impl Ord for VersionReq {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut lhs = self.comparators.iter_ref();
        let mut rhs = other.comparators.iter_ref();

        loop {
            let x = match lhs.next() {
                None => {
                    return if rhs.next().is_none() {
                        Ordering::Equal
                    } else {
                        Ordering::Less
                    };
                }
                Some(val) => val,
            };

            let y = match rhs.next() {
                None => return Ordering::Greater,
                Some(val) => val,
            };

            match (x.op as usize, x.major, x.minor, x.patch, &x.pre).cmp(&(
                y.op as usize,
                y.major,
                y.minor,
                y.patch,
                &y.pre,
            )) {
                Ordering::Equal => (),
                non_eq => return non_eq,
            }
        }
    }
}

impl PartialOrd for VersionReq {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl From<semver::VersionReq> for VersionReq {
    fn from(req: semver::VersionReq) -> Self {
        let comparators = Slice::new(&req.comparators);
        VersionReq { comparators }
    }
}

impl FromStr for VersionReq {
    type Err = semver::Error;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        semver::VersionReq::from_str(string).map(VersionReq::from)
    }
}

impl Display for VersionReq {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        if self.comparators.is_empty() {
            return formatter.write_str("*");
        }
        for (i, comparator) in self.comparators.iter_ref().enumerate() {
            if i > 0 {
                formatter.write_str(", ")?;
            }
            write!(formatter, "{}", comparator)?;
        }
        Ok(())
    }
}

impl Debug for VersionReq {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(formatter, "VersionReq({})", self)
    }
}

fn matches_req(comparators: Slice<Comparator>, ver: &Version) -> bool {
    for cmp in comparators.iter_ref() {
        if !matches_impl(cmp, ver) {
            return false;
        }
    }

    if ver.pre.is_empty() {
        return true;
    }

    // If a version has a prerelease tag (for example, 1.2.3-alpha.3) then it
    // will only be allowed to satisfy req if at least one comparator with the
    // same major.minor.patch also has a prerelease tag.
    for cmp in comparators.iter_ref() {
        if pre_is_compatible(cmp, ver) {
            return true;
        }
    }

    false
}

fn matches_impl(cmp: &Comparator, ver: &Version) -> bool {
    match cmp.op {
        Op::Exact | Op::Wildcard => matches_exact(cmp, ver),
        Op::Greater => matches_greater(cmp, ver),
        Op::GreaterEq => matches_exact(cmp, ver) || matches_greater(cmp, ver),
        Op::Less => matches_less(cmp, ver),
        Op::LessEq => matches_exact(cmp, ver) || matches_less(cmp, ver),
        Op::Tilde => matches_tilde(cmp, ver),
        Op::Caret => matches_caret(cmp, ver),
        _ => unimplemented!(),
    }
}

fn matches_exact(cmp: &Comparator, ver: &Version) -> bool {
    if ver.major != cmp.major {
        return false;
    }

    if let Some(minor) = cmp.minor {
        if ver.minor != minor {
            return false;
        }
    }

    if let Some(patch) = cmp.patch {
        if ver.patch != patch {
            return false;
        }
    }

    ver.pre == cmp.pre
}

fn matches_greater(cmp: &Comparator, ver: &Version) -> bool {
    if ver.major != cmp.major {
        return ver.major > cmp.major;
    }

    match cmp.minor {
        None => return false,
        Some(minor) => {
            if ver.minor != minor {
                return ver.minor > minor;
            }
        }
    }

    match cmp.patch {
        None => return false,
        Some(patch) => {
            if ver.patch != patch {
                return ver.patch > patch;
            }
        }
    }

    ver.pre > cmp.pre
}

fn matches_less(cmp: &Comparator, ver: &Version) -> bool {
    if ver.major != cmp.major {
        return ver.major < cmp.major;
    }

    match cmp.minor {
        None => return false,
        Some(minor) => {
            if ver.minor != minor {
                return ver.minor < minor;
            }
        }
    }

    match cmp.patch {
        None => return false,
        Some(patch) => {
            if ver.patch != patch {
                return ver.patch < patch;
            }
        }
    }

    ver.pre < cmp.pre
}

fn matches_tilde(cmp: &Comparator, ver: &Version) -> bool {
    if ver.major != cmp.major {
        return false;
    }

    if let Some(minor) = cmp.minor {
        if ver.minor != minor {
            return false;
        }
    }

    if let Some(patch) = cmp.patch {
        if ver.patch != patch {
            return ver.patch > patch;
        }
    }

    ver.pre >= cmp.pre
}

fn matches_caret(cmp: &Comparator, ver: &Version) -> bool {
    if ver.major != cmp.major {
        return false;
    }

    let minor = match cmp.minor {
        None => return true,
        Some(minor) => minor,
    };

    let patch = match cmp.patch {
        None => {
            return if cmp.major > 0 {
                ver.minor >= minor
            } else {
                ver.minor == minor
            };
        }
        Some(patch) => patch,
    };

    if cmp.major > 0 {
        if ver.minor != minor {
            return ver.minor > minor;
        } else if ver.patch != patch {
            return ver.patch > patch;
        }
    } else if minor > 0 {
        if ver.minor != minor {
            return false;
        } else if ver.patch != patch {
            return ver.patch > patch;
        }
    } else if ver.minor != minor || ver.patch != patch {
        return false;
    }

    ver.pre >= cmp.pre
}

fn pre_is_compatible(cmp: &Comparator, ver: &Version) -> bool {
    cmp.major == ver.major
        && cmp.minor == Some(ver.minor)
        && cmp.patch == Some(ver.patch)
        && !cmp.pre.is_empty()
}

use chrono::{NaiveDate, NaiveTime, Utc};
use differential_dataflow::lattice::Lattice;
use std::cmp;
use std::fmt::{self, Debug, Display};
use timely::order::{PartialOrder, TotalOrder};
use timely::progress::timestamp::{PathSummary, Refines, Timestamp};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct NaiveDateTime(chrono::NaiveDateTime);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub struct Duration(chrono::Duration);

impl NaiveDateTime {
    pub fn new(date: NaiveDate, time: NaiveTime) -> Self {
        NaiveDateTime(chrono::NaiveDateTime::new(date, time))
    }

    pub fn now() -> Self {
        NaiveDateTime(Utc::now().naive_utc())
    }

    pub fn seconds(&self) -> i64 {
        self.0.timestamp()
    }

    pub fn millis(&self) -> i64 {
        self.0.timestamp_millis()
    }

    pub fn subsec_nanos(&self) -> u32 {
        self.0.timestamp_subsec_nanos()
    }

    pub fn from_timestamp(secs: i64, nanos: u32) -> Self {
        NaiveDateTime(chrono::NaiveDateTime::from_timestamp(secs, nanos))
    }
}

impl From<chrono::NaiveDateTime> for NaiveDateTime {
    fn from(naive_date_time: chrono::NaiveDateTime) -> Self {
        NaiveDateTime(naive_date_time)
    }
}

impl Timestamp for NaiveDateTime {
    type Summary = Duration;

    fn minimum() -> Self {
        NaiveDateTime(chrono::NaiveDateTime::from_timestamp(0, 0))
    }
}

impl Lattice for NaiveDateTime {
    fn join(&self, other: &Self) -> Self {
        cmp::max(*self, *other)
    }

    fn meet(&self, other: &Self) -> Self {
        cmp::min(*self, *other)
    }
}

impl PartialOrder for NaiveDateTime {
    fn less_than(&self, other: &Self) -> bool {
        self < other
    }

    fn less_equal(&self, other: &Self) -> bool {
        self <= other
    }
}

impl TotalOrder for NaiveDateTime {}

impl PathSummary<NaiveDateTime> for Duration {
    fn results_in(&self, src: &NaiveDateTime) -> Option<NaiveDateTime> {
        src.0.checked_add_signed(self.0).map(NaiveDateTime)
    }

    fn followed_by(&self, other: &Self) -> Option<Self> {
        self.0.checked_add(&other.0).map(Duration)
    }
}

impl Refines<()> for NaiveDateTime {
    fn to_inner(_other: ()) -> Self {
        Self::minimum()
    }

    #[allow(clippy::unused_unit)]
    fn to_outer(self) -> () {}

    #[allow(clippy::unused_unit)]
    fn summarize(_path: <Self as Timestamp>::Summary) -> () {}
}

impl PartialOrder for Duration {
    fn less_than(&self, other: &Self) -> bool {
        self < other
    }

    fn less_equal(&self, other: &Self) -> bool {
        self <= other
    }
}

impl Default for NaiveDateTime {
    fn default() -> Self {
        Self::minimum()
    }
}

impl Display for NaiveDateTime {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

impl Debug for NaiveDateTime {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&self.0, formatter)
    }
}

impl Default for Duration {
    fn default() -> Self {
        Duration(chrono::Duration::nanoseconds(0))
    }
}

impl Debug for Duration {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&self.0, formatter)
    }
}

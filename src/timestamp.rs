use chrono::{NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use differential_dataflow::lattice::Lattice;
use std::cmp;
use std::fmt::{self, Debug, Display};
use timely::order::{PartialOrder, TotalOrder};
use timely::progress::timestamp::{PathSummary, Refines, Timestamp};

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
#[repr(transparent)]
pub struct DateTime(chrono::DateTime<Utc>);

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[repr(transparent)]
pub struct Duration(chrono::Duration);

impl DateTime {
    pub fn new(date: NaiveDate, time: NaiveTime) -> Self {
        DateTime(Utc.from_utc_datetime(&NaiveDateTime::new(date, time)))
    }

    pub fn now() -> Self {
        DateTime(Utc::now())
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
        DateTime(Utc.from_utc_datetime(&NaiveDateTime::from_timestamp_opt(secs, nanos).unwrap()))
    }
}

impl From<chrono::DateTime<Utc>> for DateTime {
    fn from(date_time: chrono::DateTime<Utc>) -> Self {
        DateTime(date_time)
    }
}

impl Timestamp for DateTime {
    type Summary = Duration;

    fn minimum() -> Self {
        Self::from_timestamp(0, 0)
    }
}

impl Lattice for DateTime {
    fn join(&self, other: &Self) -> Self {
        cmp::max(*self, *other)
    }

    fn meet(&self, other: &Self) -> Self {
        cmp::min(*self, *other)
    }
}

impl PartialOrder for DateTime {
    fn less_than(&self, other: &Self) -> bool {
        self < other
    }

    fn less_equal(&self, other: &Self) -> bool {
        self <= other
    }
}

impl TotalOrder for DateTime {}

impl PathSummary<DateTime> for Duration {
    fn results_in(&self, src: &DateTime) -> Option<DateTime> {
        src.0.checked_add_signed(self.0).map(DateTime)
    }

    fn followed_by(&self, other: &Self) -> Option<Self> {
        self.0.checked_add(&other.0).map(Duration)
    }
}

impl Refines<()> for DateTime {
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

impl Default for DateTime {
    fn default() -> Self {
        Self::minimum()
    }
}

impl Display for DateTime {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&self.0, formatter)
    }
}

impl Debug for DateTime {
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

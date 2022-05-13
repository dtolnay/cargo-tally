macro_rules! const_assert_eq {
    ($left:expr, $right:expr) => {
        const _: [(); $left as usize] = [(); $right as usize];
    };
}

macro_rules! const_assert {
    ($($cond:expr),* $(,)?) => {
        const_assert_eq!($($cond)&&*, true);
    };
}

macro_rules! version {
    ($major_minor:tt . $patch:tt) => {{
        const major_minor: &'static [u8] = stringify!($major_minor).as_bytes();
        const_assert! {
            major_minor.len() == 3,
            major_minor[0] >= b'0' && major_minor[0] <= b'9',
            major_minor[1] == b'.',
            major_minor[2] >= b'0' && major_minor[2] <= b'9',
        }
        cargo_tally::version::Version(semver::Version {
            major: (major_minor[0] - b'0') as u64,
            minor: (major_minor[2] - b'0') as u64,
            patch: $patch,
            pre: semver::Prerelease::EMPTY,
            build: semver::BuildMetadata::EMPTY,
        })
    }};
}

macro_rules! version_req {
    (^ $major_minor:tt) => {{
        const major_minor: &'static [u8] = stringify!($major_minor).as_bytes();
        const_assert! {
            major_minor.len() == 3,
            major_minor[0] >= b'0' && major_minor[0] <= b'9',
            major_minor[1] == b'.',
            major_minor[2] >= b'0' && major_minor[2] <= b'9',
        }
        const comparators: &'static [semver::Comparator] = &[semver::Comparator {
            op: semver::Op::Caret,
            major: (major_minor[0] - b'0') as u64,
            minor: Some((major_minor[2] - b'0') as u64),
            patch: None,
            pre: semver::Prerelease::EMPTY,
        }];
        cargo_tally::version::VersionReq {
            comparators: cargo_tally::arena::Slice::from(comparators),
        }
    }};
}

macro_rules! datetime {
    ($day:tt $month:ident $year:tt $hour:tt : $min:tt : $sec:tt) => {{
        const_assert! {
            $day >= 1 && $day <= 31,
            $year >= 2014,
            $hour >= 0 && $hour <= 23,
            $min >= 0 && $min <= 59,
            $sec >= 0 && $sec <= 60,
        }
        cargo_tally::timestamp::NaiveDateTime::new(
            chrono::NaiveDate::from_ymd($year, month_number!($month), $day),
            chrono::NaiveTime::from_hms($hour, $min, $sec),
        )
    }};
}

#[rustfmt::skip]
#[allow(unused_macro_rules)]
macro_rules! month_number {
    (Jan) => { 1 };
    (Feb) => { 2 };
    (Mar) => { 3 };
    (Apr) => { 4 };
    (May) => { 5 };
    (Jun) => { 6 };
    (Jul) => { 7 };
    (Aug) => { 8 };
    (Sep) => { 9 };
    (Oct) => { 10 };
    (Nov) => { 11 };
    (Dec) => { 12 };
}

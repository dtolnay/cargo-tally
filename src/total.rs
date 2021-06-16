use cargo_tally::timestamp::NaiveDateTime;
use cargo_tally::Release;
use std::collections::BTreeSet as Set;

pub(crate) struct Total {
    times: Vec<NaiveDateTime>,
}

impl Total {
    pub(crate) fn index(releases: &[Release]) -> Self {
        let mut crate_ids = Set::new();
        let mut times = Vec::new();
        for release in releases {
            if crate_ids.insert(release.crate_id) {
                times.push(release.created_at);
            }
        }
        Total { times }
    }

    pub(crate) fn eval(&self, time: NaiveDateTime) -> u32 {
        match self.times.binary_search(&time) {
            Ok(i) => 1 + i as u32,
            Err(i) => i as u32,
        }
    }
}

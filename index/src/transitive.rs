use serde::{Deserialize, Deserializer, Serialize, Serializer};

use cargo_tally::Crate;

#[derive(Serialize, Deserialize)]
pub struct TranitiveCrateDeps {
    pub name: String,
    /// Crates that depend on this crate
    depended_on: Vec<Someting>,
    // or we could just use the count
    // count: usize,
}

impl TranitiveCrateDeps {
    // ? use tally's Resolve and Universe to compute?
    fn calculate_trans_deps(&mut self, crates: &[Crate]) {
        // calculate from Vec<Crate>
    }
}

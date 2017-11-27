use tally::{CrateKey, Universe};

use std::fmt::{self, Debug};

/// Format an IntoIterator of crate keys as:
///
///     [serde:1.0.0, serde:1.0.1, serde:1.0.2]
pub(crate) struct CrateCollection<'a, I> {
    universe: &'a Universe,
    crates: I,
}

impl<'a, I> CrateCollection<'a, I> {
    pub(crate) fn new(universe: &'a Universe, crates: I) -> Self {
        CrateCollection { universe, crates }
    }
}

impl<'a, I> Debug for CrateCollection<'a, I>
where
    I: Clone + IntoIterator<Item = &'a CrateKey>,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let universe = self.universe;
        let crates = self.crates
            .clone()
            .into_iter()
            .map(|&key| DebugCrate { universe, key });
        formatter.debug_list().entries(crates).finish()
    }
}

/// Format a single crate key as `serde:1.0.0`.
struct DebugCrate<'a> {
    universe: &'a Universe,
    key: CrateKey,
}

impl<'a> Debug for DebugCrate<'a> {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        let name = self.key.name;
        let num = &self.universe.crates[&name][self.key.index as usize].num;
        write!(formatter, "{}:{}", name, num)
    }
}

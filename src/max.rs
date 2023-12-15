use crate::hint::TypeHint;
use crate::present::Present;
use differential_dataflow::collection::Collection;
use differential_dataflow::difference::{Multiply, Semigroup};
use differential_dataflow::lattice::Lattice;
use differential_dataflow::operators::CountTotal;
use differential_dataflow::ExchangeData;
use std::fmt::Debug;
use std::hash::Hash;
use std::iter::once;
use timely::dataflow::Scope;
use timely::order::TotalOrder;

pub(crate) trait MaxByKey<G, K, V, R>
where
    G: Scope,
{
    fn max_by_key(&self) -> Collection<G, (K, V), isize>;
}

impl<G, K, V, R> MaxByKey<G, K, V, R> for Collection<G, (K, V), R>
where
    G: Scope,
    K: Clone + ExchangeData + Hash,
    V: Clone + Ord + ExchangeData + Debug,
    R: Semigroup,
    Max<V>: Multiply<R, Output = Max<V>>,
    G::Timestamp: TotalOrder + Lattice,
{
    fn max_by_key(&self) -> Collection<G, (K, V), isize> {
        self.explode(|(key, value)| once((key, Max { value })))
            .T::<K>()
            .count_total()
            .KV::<K, Max<V>>()
            .map(|(key, max)| (key, max.value))
    }
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub(crate) struct Max<T> {
    value: T,
}

impl<T> Multiply<Present> for Max<T> {
    type Output = Self;

    fn multiply(self, rhs: &Present) -> Self::Output {
        let _ = rhs;
        self
    }
}

impl<T> Semigroup for Max<T>
where
    T: Ord + Clone + Debug + 'static,
{
    fn plus_equals(&mut self, rhs: &Self) {
        if self.value < rhs.value {
            self.value = rhs.value.clone();
        }
    }

    fn is_zero(&self) -> bool {
        false
    }
}

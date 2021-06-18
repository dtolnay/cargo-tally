use crate::present::Present;
use differential_dataflow::difference::Semigroup;
use std::fmt::Debug;
use std::ops::{AddAssign, Mul};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub struct Max<T> {
    pub value: T,
}

impl<T> Max<T> {
    pub fn new(value: T) -> Self {
        Self { value }
    }
}

impl<T> Mul<Present> for Max<T> {
    type Output = Self;

    fn mul(self, rhs: Present) -> Self::Output {
        let _ = rhs;
        self
    }
}

impl<T> AddAssign<&Self> for Max<T>
where
    T: Ord + Clone,
{
    fn add_assign(&mut self, rhs: &Self) {
        if self.value < rhs.value {
            self.value = rhs.value.clone();
        }
    }
}

impl<T> Semigroup for Max<T>
where
    T: Ord + Clone + Debug + 'static,
{
    fn is_zero(&self) -> bool {
        false
    }
}

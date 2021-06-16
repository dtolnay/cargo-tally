use differential_dataflow::difference::Semigroup;
use std::ops::{AddAssign, Mul};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub(crate) struct Present;

impl Semigroup for Present {
    fn is_zero(&self) -> bool {
        false
    }
}

impl AddAssign<&Present> for Present {
    fn add_assign(&mut self, rhs: &Present) {
        let _ = rhs;
    }
}

impl Mul<Present> for Present {
    type Output = Present;

    fn mul(self, rhs: Present) -> Self::Output {
        let _ = rhs;
        Present
    }
}

impl Mul<Present> for isize {
    type Output = isize;

    fn mul(self, rhs: Present) -> Self::Output {
        let _ = rhs;
        self
    }
}

impl Mul<isize> for Present {
    type Output = isize;

    fn mul(self, rhs: isize) -> Self::Output {
        rhs
    }
}

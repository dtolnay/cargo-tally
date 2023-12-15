use differential_dataflow::difference::{Multiply, Semigroup};

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Debug)]
pub(crate) struct Present;

impl Semigroup for Present {
    fn plus_equals(&mut self, rhs: &Present) {
        let _ = rhs;
    }

    fn is_zero(&self) -> bool {
        false
    }
}

impl Multiply<Present> for Present {
    type Output = Present;

    fn multiply(self, rhs: &Present) -> Self::Output {
        let _ = rhs;
        Present
    }
}

impl Multiply<Present> for isize {
    type Output = isize;

    fn multiply(self, rhs: &Present) -> Self::Output {
        let _ = rhs;
        self
    }
}

impl Multiply<isize> for Present {
    type Output = isize;

    fn multiply(self, rhs: &isize) -> Self::Output {
        *rhs
    }
}

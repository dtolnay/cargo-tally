use std::fmt::Display;

pub trait DisplayUnwrap {
    type Output;
    fn display_unwrap(self) -> Self::Output;
}

impl<T, E> DisplayUnwrap for Result<T, E>
    where E: Display
{
    type Output = T;

    fn display_unwrap(self) -> T {
        self.unwrap_or_else(|err| panic!(err.to_string()))
    }
}

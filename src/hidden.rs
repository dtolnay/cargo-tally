// There is no library public API. Only the command line tool is considered
// public API.

#[path = "lib.rs"]
mod lib;

#[doc(hidden)]
pub use crate::lib::*;

//! TODO figure how to split into files

use fnv::FnvBuildHasher;
use lazy_static::lazy_static;
use string_interner::{StringInterner, Symbol};

use std::cell::UnsafeCell;
use std::fmt::{self, Debug, Display};
use std::marker::PhantomData;
use std::ops::Deref;


lazy_static! {
    static ref INTERN: SymbolsStayOnOneThread = Default::default();
}

struct SymbolsStayOnOneThread {
    interner: UnsafeCell<StringInterner<CrateName, FnvBuildHasher>>,
}

impl Default for SymbolsStayOnOneThread {
    fn default() -> Self {
        SymbolsStayOnOneThread {
            interner: UnsafeCell::new(StringInterner::with_hasher(Default::default())),
        }
    }
}

unsafe impl Send for SymbolsStayOnOneThread {}
unsafe impl Sync for SymbolsStayOnOneThread {}

#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, super::Serialize, super::Deserialize)]
pub struct CrateName {
    value: u32,
    not_send_sync: PhantomData<*const ()>,
}

pub fn crate_name<T: Into<String> + AsRef<str>>(string: T) -> CrateName {
    let c = INTERN.interner.get();
    let c = unsafe { &mut *c };
    c.get_or_intern(string)
}

impl Symbol for CrateName {
    fn from_usize(value: usize) -> Self {
        CrateName {
            value: value as u32,
            not_send_sync: PhantomData,
        }
    }
    fn to_usize(self) -> usize {
        self.value as usize
    }
}

impl Deref for CrateName {
    type Target = str;

    fn deref(&self) -> &str {
        let c = INTERN.interner.get();
        unsafe {
            let c = &*c;
            c.resolve_unchecked(*self)
        }
    }
}

impl Debug for CrateName {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(&**self, formatter)
    }
}

impl Display for CrateName {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(&**self, formatter)
    }
}

impl<'a> PartialEq<&'a str> for CrateName {
    fn eq(&self, other: &&str) -> bool {
        &**self == *other
    }
}

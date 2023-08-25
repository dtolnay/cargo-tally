use fnv::FnvHashMap;
use std::any::TypeId;
use std::fmt::{self, Debug};
use std::iter::{Copied, FromIterator};
use std::slice::Iter;
use std::sync::OnceLock;
use std::sync::{Mutex, PoisonError};
use typed_arena::Arena;

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct Slice<T: 'static> {
    contents: &'static [T],
}

impl<T> Slice<T>
where
    T: 'static,
{
    pub const EMPTY: Self = Slice { contents: &[] };

    pub fn new(slice: &[T]) -> Self
    where
        T: Send + Clone,
    {
        slice.iter().cloned().collect()
    }

    pub const fn from(contents: &'static [T]) -> Self {
        Slice { contents }
    }

    pub fn iter(&self) -> impl Iterator<Item = T>
    where
        T: Copy,
    {
        (*self).into_iter()
    }

    pub fn iter_ref(&self) -> impl Iterator<Item = &'static T> {
        self.contents.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.contents.is_empty()
    }
}

impl<T> Copy for Slice<T> where T: 'static {}

impl<T> Clone for Slice<T>
where
    T: 'static,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> FromIterator<T> for Slice<T>
where
    T: 'static + Send + Clone,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
    {
        let iter = iter.into_iter();
        if iter.size_hint() == (0, Some(0)) {
            return Slice::EMPTY;
        }

        static ARENA: OnceLock<Mutex<FnvHashMap<TypeId, Box<dyn Send>>>> = OnceLock::new();

        let mut map = ARENA
            .get_or_init(Mutex::default)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let arena: &Box<dyn Send> = map
            .entry(TypeId::of::<T>())
            .or_insert_with(|| Box::new(Arena::<T>::new()));
        let arena = unsafe { &*(&**arena as *const dyn Send as *const Arena<T>) };
        Slice {
            contents: arena.alloc_extend(iter),
        }
    }
}

impl<T> IntoIterator for Slice<T>
where
    T: 'static + Copy,
{
    type Item = T;
    type IntoIter = Copied<Iter<'static, T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.contents.iter().copied()
    }
}

impl<T> Debug for Slice<T>
where
    T: 'static + Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self.contents, formatter)
    }
}

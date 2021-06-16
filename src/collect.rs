use differential_dataflow::collection::Collection;
use differential_dataflow::difference::Semigroup;
use std::mem;
use std::sync::{Arc, Mutex, PoisonError};
use timely::dataflow::Scope;
use timely::Data;

pub(crate) trait Collect<T> {
    fn collect_into(&self, result: &Emitter<T>);
}

pub(crate) struct ResultCollection<T> {
    out: Arc<Mutex<Vec<T>>>,
}

pub(crate) struct Emitter<T> {
    out: Arc<Mutex<Vec<T>>>,
}

impl<T> ResultCollection<T> {
    pub(crate) fn new() -> Self {
        let out = Arc::new(Mutex::new(Vec::new()));
        ResultCollection { out }
    }

    pub(crate) fn emitter(&self) -> Emitter<T> {
        let out = Arc::clone(&self.out);
        Emitter { out }
    }
}

impl<D, T, R> ResultCollection<(D, T, R)>
where
    T: Ord,
{
    pub(crate) fn sort(&self) {
        self.out
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .sort_by(
                |(_ldata, ltimestamp, _ldiff), (_rdata, rtimestamp, _rdiff)| {
                    ltimestamp.cmp(rtimestamp)
                },
            );
    }
}

impl<G, D, R> Collect<(D, G::Timestamp, R)> for Collection<G, D, R>
where
    G: Scope,
    D: Data,
    R: Semigroup,
    G::Timestamp: Data,
{
    fn collect_into(&self, result: &Emitter<(D, G::Timestamp, R)>) {
        let out = Arc::clone(&result.out);
        self.inspect_batch(move |_timestamp, slice| {
            out.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(slice);
        });
    }
}

impl<T> IntoIterator for ResultCollection<T> {
    type Item = T;
    type IntoIter = <Vec<T> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        let mut out = self.out.lock().unwrap_or_else(PoisonError::into_inner);
        mem::take(&mut *out).into_iter()
    }
}

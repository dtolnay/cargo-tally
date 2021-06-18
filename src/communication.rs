// As far as I can tell, timely dataflow uses abomonation only for interprocess
// communication. Within a single process, it uses the Clone impl instead. We
// stub out the Abomonation impl since it will never be called.
macro_rules! do_not_abomonate {
    ($($path:ident)::+ $(<$param:ident>)? $(where $($clause:tt)*)?) => {
        impl $(<$param>)? abomonation::Abomonation for $($path)::+ $(<$param>)? $(where $($clause)*)? {
            unsafe fn entomb<W: std::io::Write>(&self, _write: &mut W) -> std::io::Result<()> {
                unimplemented!("unexpected abomonation entomb");
            }
            unsafe fn exhume<'a, 'b>(&'a mut self, _bytes: &'b mut [u8]) -> Option<&'b mut [u8]> {
                // Unwinding here is unsound because abomonation would have
                // blitted the source bytes into the destination with dangling
                // pointers, and is now relying on exhume to fix it up into a
                // valid object. We abort instead.
                std::process::exit(1);
            }
            fn extent(&self) -> usize {
                unimplemented!("unexpected abomonation extent");
            }
        }
    };
}

do_not_abomonate!(crate::Dependency);
do_not_abomonate!(crate::Release);
do_not_abomonate!(crate::arena::Slice<T> where T: 'static);
do_not_abomonate!(crate::feature::CrateFeature);
do_not_abomonate!(crate::feature::DefaultFeatures);
do_not_abomonate!(crate::feature::FeatureId);
do_not_abomonate!(crate::feature::VersionFeature);
do_not_abomonate!(crate::id::CrateId);
do_not_abomonate!(crate::id::DependencyId);
do_not_abomonate!(crate::id::QueryId);
do_not_abomonate!(crate::id::VersionId);
do_not_abomonate!(crate::present::Present);
do_not_abomonate!(crate::query::Query);
do_not_abomonate!(crate::timestamp::NaiveDateTime);
do_not_abomonate!(crate::version::Version);
do_not_abomonate!(crate::version::VersionReq);

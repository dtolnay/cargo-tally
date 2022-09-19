macro_rules! stream {
    ($k:ty => $v:ty; $r:ty) => {
        stream![($k, $v); $r]
    };
    ($d:ty; $r:ty) => {
        differential_dataflow::collection::Collection<
            timely::dataflow::scopes::Child<
                'a,
                timely::worker::Worker<timely::communication::allocator::Process>,
                crate::timestamp::NaiveDateTime,
            >,
            $d,
            $r,
        >
    };
}

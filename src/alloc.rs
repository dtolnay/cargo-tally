use bytesize::ByteSize;
use std::alloc::{GlobalAlloc, Layout, System};
use std::fmt::{self, Display};
use std::sync::atomic::{AtomicU64, Ordering};

struct Allocator<A = System> {
    alloc: A,
    count: AtomicU64,
    total: AtomicU64,
    current: AtomicU64,
    peak: AtomicU64,
}

#[global_allocator]
static ALLOC: Allocator = Allocator {
    alloc: System,
    count: AtomicU64::new(0),
    total: AtomicU64::new(0),
    current: AtomicU64::new(0),
    peak: AtomicU64::new(0),
};

unsafe impl<A> GlobalAlloc for Allocator<A>
where
    A: GlobalAlloc,
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.count.fetch_add(1, Ordering::Relaxed);
        let ptr = self.alloc.alloc(layout);
        let size = layout.size() as u64;
        self.total.fetch_add(size, Ordering::Relaxed);
        let prev = self.current.fetch_add(size, Ordering::Relaxed);
        self.peak.fetch_max(prev + size, Ordering::Relaxed);
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        self.alloc.dealloc(ptr, layout);
        let size = layout.size() as u64;
        self.current.fetch_sub(size, Ordering::Relaxed);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.count.fetch_add(1, Ordering::Relaxed);
        let ptr = self.alloc.alloc_zeroed(layout);
        let size = layout.size() as u64;
        self.total.fetch_add(size, Ordering::Relaxed);
        let prev = self.current.fetch_add(size, Ordering::Relaxed);
        self.peak.fetch_max(prev + size, Ordering::Relaxed);
        ptr
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        self.count.fetch_add(1, Ordering::Relaxed);
        let new_ptr = self.alloc.realloc(ptr, layout, new_size);
        let size = layout.size() as u64;
        let new_size = new_size as u64;
        if new_ptr == ptr {
            if new_size > size {
                self.total.fetch_add(new_size - size, Ordering::Relaxed);
                let prev = self.current.fetch_add(new_size - size, Ordering::Relaxed);
                self.peak
                    .fetch_max(prev + new_size - size, Ordering::Relaxed);
            } else {
                self.current.fetch_sub(size - new_size, Ordering::Relaxed);
            }
        } else {
            self.total.fetch_add(new_size, Ordering::Relaxed);
            let prev = if new_size > size {
                self.current.fetch_add(new_size - size, Ordering::Relaxed)
            } else {
                self.current.fetch_sub(size - new_size, Ordering::Relaxed)
            };
            self.peak.fetch_max(prev + new_size, Ordering::Relaxed);
        }
        new_ptr
    }
}

pub(crate) struct AllocStat {
    count: u64,
    total: ByteSize,
    peak: ByteSize,
}

pub(crate) fn stat() -> AllocStat {
    AllocStat {
        count: ALLOC.count.load(Ordering::Relaxed),
        total: ByteSize::b(ALLOC.total.load(Ordering::Relaxed) as u64),
        peak: ByteSize::b(ALLOC.peak.load(Ordering::Relaxed) as u64),
    }
}

impl Display for AllocStat {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        write!(
            formatter,
            "{} allocations, total {}, peak {}",
            self.count, self.total, self.peak,
        )
    }
}

use bytesize::ByteSize;
use std::alloc::{self, GlobalAlloc, Layout, System};
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

const LIMIT: Option<u64> = include!(concat!(env!("OUT_DIR"), "/limit.mem"));

unsafe impl<A> GlobalAlloc for Allocator<A>
where
    A: GlobalAlloc,
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.count.fetch_add(1, Ordering::Relaxed);

        let size = layout.size() as u64;
        let prev = self.current.fetch_add(size, Ordering::Relaxed);
        self.total.fetch_add(size, Ordering::Relaxed);
        let peak = self
            .peak
            .fetch_max(prev + size, Ordering::Relaxed)
            .max(prev + size);

        if let Some(limit) = LIMIT {
            if peak > limit {
                alloc::handle_alloc_error(layout);
            }
        }

        unsafe { self.alloc.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { self.alloc.dealloc(ptr, layout) };

        let size = layout.size() as u64;
        self.current.fetch_sub(size, Ordering::Relaxed);
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.count.fetch_add(1, Ordering::Relaxed);

        let size = layout.size() as u64;
        let prev = self.current.fetch_add(size, Ordering::Relaxed);
        self.total.fetch_add(size, Ordering::Relaxed);
        let peak = self
            .peak
            .fetch_max(prev + size, Ordering::Relaxed)
            .max(prev + size);

        if let Some(limit) = LIMIT {
            if peak > limit {
                alloc::handle_alloc_error(layout);
            }
        }

        unsafe { self.alloc.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, old_layout: Layout, new_size: usize) -> *mut u8 {
        self.count.fetch_add(1, Ordering::Relaxed);

        let align = old_layout.align();
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, align) };

        let new_ptr = unsafe { self.alloc.realloc(ptr, old_layout, new_size) };
        let old_size = old_layout.size() as u64;
        let new_size = new_size as u64;

        let peak = if new_ptr == ptr {
            if new_size > old_size {
                self.total.fetch_add(new_size - old_size, Ordering::Relaxed);
                let prev = self
                    .current
                    .fetch_add(new_size - old_size, Ordering::Relaxed);
                self.peak
                    .fetch_max(prev + new_size - old_size, Ordering::Relaxed)
                    .max(prev + new_size - old_size)
            } else {
                self.current
                    .fetch_sub(old_size - new_size, Ordering::Relaxed);
                0
            }
        } else {
            self.total.fetch_add(new_size, Ordering::Relaxed);
            let prev = if new_size > old_size {
                self.current
                    .fetch_add(new_size - old_size, Ordering::Relaxed)
            } else {
                self.current
                    .fetch_sub(old_size - new_size, Ordering::Relaxed)
            };
            self.peak
                .fetch_max(prev + new_size, Ordering::Relaxed)
                .max(prev + new_size)
        };

        if let Some(limit) = LIMIT {
            if peak > limit {
                alloc::handle_alloc_error(new_layout);
            }
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
        total: ByteSize::b(ALLOC.total.load(Ordering::Relaxed)),
        peak: ByteSize::b(ALLOC.peak.load(Ordering::Relaxed)),
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

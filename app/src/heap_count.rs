//! #2138, not for merge: the Rust UI's live heap, counted, and logged every 5 s with the Unix time,
//! for the perf run's memory snapshots to be read against.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering::Relaxed};

pub struct Counting;

static BYTES: AtomicIsize = AtomicIsize::new(0);
static BLOCKS: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(layout) };
        if !p.is_null() {
            BYTES.fetch_add(layout.size() as isize, Relaxed);
            BLOCKS.fetch_add(1, Relaxed);
        }
        p
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let p = unsafe { System.alloc_zeroed(layout) };
        if !p.is_null() {
            BYTES.fetch_add(layout.size() as isize, Relaxed);
            BLOCKS.fetch_add(1, Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, layout: Layout) {
        unsafe { System.dealloc(p, layout) };
        BYTES.fetch_sub(layout.size() as isize, Relaxed);
        BLOCKS.fetch_sub(1, Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, layout: Layout, new: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, layout, new) };
        if !q.is_null() {
            BYTES.fetch_add(new as isize - layout.size() as isize, Relaxed);
        }
        q
    }
}

pub fn start_logging() {
    static STARTED: std::sync::Once = std::sync::Once::new();
    STARTED.call_once(|| {
        std::thread::spawn(|| {
            loop {
                std::thread::sleep(std::time::Duration::from_secs(5));
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0., |d| d.as_secs_f64());
                crate::log!(
                    "browser: rust heap at {now:.0}: {:.1} MiB in {} blocks",
                    BYTES.load(Relaxed) as f64 / 1048576.,
                    BLOCKS.load(Relaxed)
                );
            }
        });
    });
}

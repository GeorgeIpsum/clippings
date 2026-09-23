#![allow(dead_code)]
//! A counting global allocator shared by the memory examples: it wraps
//! `System` and counts live bytes, both as requested and, on macOS, as
//! malloc reserved them (`malloc_size`, which adds size-class rounding).

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(target_os = "macos")]
extern "C" {
    fn malloc_size(ptr: *const std::ffi::c_void) -> usize;
}

/// Bytes malloc reserved for `p`, or `size` where that cannot be asked.
#[cfg(target_os = "macos")]
fn usable(p: *mut u8, _size: usize) -> usize {
    // SAFETY: `p` is a live pointer returned by the system allocator.
    unsafe { malloc_size(p as *const std::ffi::c_void) }
}

#[cfg(not(target_os = "macos"))]
fn usable(_p: *mut u8, size: usize) -> usize {
    size
}

struct Counting;

pub static LIVE: AtomicUsize = AtomicUsize::new(0);
pub static PEAK: AtomicUsize = AtomicUsize::new(0);
pub static LIVE_USABLE: AtomicUsize = AtomicUsize::new(0);
pub static PEAK_USABLE: AtomicUsize = AtomicUsize::new(0);
pub static ALLOCS: AtomicUsize = AtomicUsize::new(0);

fn add(size: usize, real: usize) {
    let now = LIVE.fetch_add(size, Ordering::Relaxed) + size;
    PEAK.fetch_max(now, Ordering::Relaxed);
    let now = LIVE_USABLE.fetch_add(real, Ordering::Relaxed) + real;
    PEAK_USABLE.fetch_max(now, Ordering::Relaxed);
    ALLOCS.fetch_add(1, Ordering::Relaxed);
}

fn sub(size: usize, real: usize) {
    LIVE.fetch_sub(size, Ordering::Relaxed);
    LIVE_USABLE.fetch_sub(real, Ordering::Relaxed);
    ALLOCS.fetch_sub(1, Ordering::Relaxed);
}

// SAFETY: every call forwards to `System` unchanged; the counters only
// observe sizes and never touch the memory.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = System.alloc(l);
        if !p.is_null() {
            add(l.size(), usable(p, l.size()));
        }
        p
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        let p = System.alloc_zeroed(l);
        if !p.is_null() {
            add(l.size(), usable(p, l.size()));
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        sub(l.size(), usable(p, l.size()));
        System.dealloc(p, l)
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        let old_real = usable(p, l.size());
        let q = System.realloc(p, l, new);
        if !q.is_null() {
            sub(l.size(), old_real);
            add(new, usable(q, new));
        }
        q
    }
}

#[global_allocator]
static GLOBAL: Counting = Counting;

#[derive(Clone, Copy)]
pub struct Snap {
    pub live: usize,
    pub real: usize,
    pub allocs: usize,
}

pub fn snap() -> Snap {
    Snap {
        live: LIVE.load(Ordering::Relaxed),
        real: LIVE_USABLE.load(Ordering::Relaxed),
        allocs: ALLOCS.load(Ordering::Relaxed),
    }
}

/// Resets the peaks to the current live values and returns them.
pub fn reset_peak() -> Snap {
    let s = snap();
    PEAK.store(s.live, Ordering::Relaxed);
    PEAK_USABLE.store(s.real, Ordering::Relaxed);
    s
}

/// Peak above `base` since the last [`reset_peak`], requested and reserved.
pub fn peak_above(base: Snap) -> (usize, usize) {
    (
        PEAK.load(Ordering::Relaxed).saturating_sub(base.live),
        PEAK_USABLE
            .load(Ordering::Relaxed)
            .saturating_sub(base.real),
    )
}

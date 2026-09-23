//! `clippings lsp` under the counting allocator, for
//! docs/benchmarks/2026-09-memory.md: runs the same server loop as the
//! release binary (`server::main_loop::run_stdio`) and, from a side thread,
//! appends one line every 20 ms to the file named by `CLIPPINGS_MEMLOG`:
//!
//! ```text
//! <ms since start> <live bytes requested> <live bytes reserved> <allocations> <zone bytes in use> <zone bytes held>
//! ```
//!
//! The last two come from `malloc_zone_statistics` on macOS: the bytes
//! malloc's zones hand out, and the bytes they hold from the system, which
//! includes freed memory they have not returned. With
//! `CLIPPINGS_MEMLOG_RELIEF_MS=N` the thread also calls
//! `malloc_zone_pressure_relief` every N ms, to show how much of that
//! retained memory the allocator can give back.
//!
//! `scripts/memprofile.py session --binary target/release/examples/memprofile_lsp`
//! drives it like the release server.

#[path = "support/counting.rs"]
mod counting;

use std::io::Write;
use std::time::{Duration, Instant};

#[cfg(target_os = "macos")]
mod zone {
    #[repr(C)]
    #[derive(Default)]
    struct MallocStatistics {
        blocks_in_use: u32,
        size_in_use: usize,
        max_size_in_use: usize,
        size_allocated: usize,
    }

    extern "C" {
        fn malloc_zone_statistics(zone: *mut std::ffi::c_void, stats: *mut MallocStatistics);
        fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
    }

    /// Bytes in use and bytes held by all malloc zones.
    pub fn stats() -> (usize, usize) {
        let mut s = MallocStatistics::default();
        // SAFETY: a null zone asks for the sum over all zones; `s` is a
        // valid, correctly laid out `malloc_statistics_t`.
        unsafe { malloc_zone_statistics(std::ptr::null_mut(), &mut s) };
        (s.size_in_use, s.size_allocated)
    }

    /// Asks every zone to return as much free memory as it can.
    pub fn relieve() -> usize {
        // SAFETY: a null zone and a zero goal mean all zones, as much as possible.
        unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) }
    }
}

#[cfg(not(target_os = "macos"))]
mod zone {
    pub fn stats() -> (usize, usize) {
        (0, 0)
    }
    pub fn relieve() -> usize {
        0
    }
}

fn main() -> Result<(), String> {
    if let Ok(path) = std::env::var("CLIPPINGS_MEMLOG") {
        let relief = std::env::var("CLIPPINGS_MEMLOG_RELIEF_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .map(Duration::from_millis);
        let mut file = std::fs::File::create(&path).map_err(|e| e.to_string())?;
        std::thread::spawn(move || {
            let start = Instant::now();
            let mut last_relief = start;
            loop {
                if relief.is_some_and(|r| last_relief.elapsed() >= r) {
                    zone::relieve();
                    last_relief = Instant::now();
                }
                let s = counting::snap();
                let (in_use, held) = zone::stats();
                let _ = writeln!(
                    file,
                    "{} {} {} {} {} {}",
                    start.elapsed().as_millis(),
                    s.live,
                    s.real,
                    s.allocs,
                    in_use,
                    held
                );
                std::thread::sleep(Duration::from_millis(20));
            }
        });
    }
    clippings_core::server::main_loop::run_stdio()
}

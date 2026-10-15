//! Peak heap of one workload on cpat-core: the most live bytes while the
//! grammar is built (and AutoComplete made), and the most live bytes above
//! that while one iteration runs. Peak RSS can't say this: macOS's malloc
//! keeps freed pages, so a process's RSS grows with the number of
//! iterations even though every parse frees what it made.
//!
//!   cpat-bench-peak <out.json> <workload.json>

use std::alloc::{GlobalAlloc, Layout, System};
use std::process::ExitCode;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use serde_json::json;

use cpat_bench::workload::{Kind, Workload};
use cpat_bench::write;

/// The system allocator, counting live bytes and their high-water mark.
struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);

fn grew(by: usize) {
    let live = LIVE.fetch_add(by, Relaxed) + by;
    PEAK.fetch_max(live, Relaxed);
}

// SAFETY: every call forwards to `System` unchanged; only counters change.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        grew(layout.size());
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        grew(new_size);
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

/// Starts a new high-water mark at the current live bytes; returns them.
fn reset_peak() -> usize {
    let live = LIVE.load(Relaxed);
    PEAK.store(live, Relaxed);
    live
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [out, file] = &args[..] else {
        eprintln!("usage: cpat-bench-peak <out.json> <workload.json>");
        return ExitCode::from(2);
    };
    match peak(out, file) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn peak(out: &str, file: &str) -> Result<(), String> {
    let workload = Workload::load(file)?;
    let before = reset_peak();
    let (prepare, iteration) = match workload.kind {
        Kind::Noop => (0, 0),
        Kind::Build => {
            let built = workload.build();
            let prepare = PEAK.load(Relaxed) - before;
            drop(built);
            (prepare, prepare)
        }
        Kind::Parse | Kind::Suggest => {
            let mut prepared = workload.prepare()?;
            let prepare = PEAK.load(Relaxed) - before;
            let base = reset_peak();
            prepared.run_once();
            (prepare, PEAK.load(Relaxed) - base)
        }
    };
    write(
        out,
        &json!({ "name": workload.name, "prepareBytes": prepare, "iterationBytes": iteration }),
    )
}

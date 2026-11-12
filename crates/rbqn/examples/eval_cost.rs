//! Issue #174 measurement: per-call cost and memory retained by `Bqn::eval`
//! on one reused `Bqn`. `cargo run --release --example eval_cost -- [N] [SRC]`.
//!
//! Reports wall time per call, heap bytes still allocated per call (a counting
//! global allocator), host arrays still live per call (`live_arrays`), derived
//! objects allocated per call, and process RSS before and after.
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::time::Instant;

struct Counting;
static HEAP: AtomicIsize = AtomicIsize::new(0);

// SAFETY: forwards every call to `System` unchanged and only counts bytes.
unsafe impl GlobalAlloc for Counting {
    // SAFETY: GlobalAlloc contract; the caller upholds it and it is passed to `System` as is.
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        HEAP.fetch_add(l.size() as isize, Ordering::Relaxed);
        // SAFETY: same contract as the caller's.
        unsafe { System.alloc(l) }
    }
    // SAFETY: GlobalAlloc contract; the caller upholds it and it is passed to `System` as is.
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        HEAP.fetch_sub(l.size() as isize, Ordering::Relaxed);
        // SAFETY: same contract as the caller's.
        unsafe { System.dealloc(p, l) }
    }
    // SAFETY: GlobalAlloc contract; the caller upholds it and it is passed to `System` as is.
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        HEAP.fetch_add(n as isize - l.size() as isize, Ordering::Relaxed);
        // SAFETY: same contract as the caller's.
        unsafe { System.realloc(p, l, n) }
    }
}

#[global_allocator]
static A: Counting = Counting;

fn rss_bytes() -> u64 {
    // Current resident set size via `ps` (portable across macOS/Linux).
    let pid = std::process::id().to_string();
    let out = std::process::Command::new("ps").args(["-o", "rss=", "-p", &pid]).output();
    out.ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

fn main() {
    let mut args = std::env::args().skip(1);
    let n: usize = args.next().and_then(|s| s.parse().ok()).unwrap_or(500);
    let src = args.next().unwrap_or_else(|| "1".to_string());
    let t0 = Instant::now();
    let bqn = rbqn::Bqn::new().unwrap();
    let boot = t0.elapsed();
    bqn.eval(&src).unwrap(); // warm up
    let (r0, h0) = (rss_bytes(), HEAP.load(Ordering::Relaxed));
    let (a0, d0) = (rbqn_core::arrstore::live_arrays(), rbqn_vm::derive::derived_allocated());
    let t = Instant::now();
    for _ in 0..n {
        bqn.eval(&src).unwrap();
    }
    let dt = t.elapsed();
    let (r1, h1) = (rss_bytes(), HEAP.load(Ordering::Relaxed));
    let (a1, d1) = (rbqn_core::arrstore::live_arrays(), rbqn_vm::derive::derived_allocated());
    let per = |x: f64| x / n as f64;
    println!(
        "boot {:.1} ms; {n} evals of {src:?}: {:.3} ms/call; heap +{:.1} KB/call; \
         live arrays +{:.0}/call; derived +{:.0}/call; RSS {:.1} -> {:.1} MB",
        boot.as_secs_f64() * 1e3,
        per(dt.as_secs_f64() * 1e3),
        per((h1 - h0) as f64 / 1e3),
        per(a1 as f64 - a0 as f64),
        per(d1 as f64 - d0 as f64),
        r0 as f64 / 1e6,
        r1 as f64 / 1e6,
    );
}

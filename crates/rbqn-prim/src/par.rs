//! Multicore helpers for typed numeric slices (issue #104).
//!
//! NOTE: the array store is thread-local (see rbqn-core arrstore). Nothing in
//! here may receive a `B` or call `get_arr`; only plain slices and fresh
//! `Vec`s cross threads. With one thread (`-t 1`, or never initialised) every
//! helper runs the same serial loop the caller had before.

use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Smallest element count that runs in parallel elementwise (measured par/serial
/// on 12 cores: 1.0-1.4 at 2^14, <= 0.96 for every op at 2^15; see PR #104).
pub const PAR_MIN: usize = 1 << 15;

/// Smallest length for a parallel F64 `⌊´ ⌈´`: a parallel region costs a fixed
/// ~40 us, so folds, one cheap read pass, need more data than elementwise ops.
pub const FOLD_MIN: usize = 1 << 17;

/// Smallest length for a parallel integer fold (`+´ × ⌊´ ⌈´`): serial integer
/// reductions run near memory bandwidth, so threads only win once the data is
/// far out of cache (measured on 12 cores: par/serial 1.03 at 2^19, 0.92 at 2^20, 0.73 at 2^21).
pub const INT_FOLD_MIN: usize = 1 << 20;

static THREADS: AtomicUsize = AtomicUsize::new(1);
static MIN_LEN: AtomicUsize = AtomicUsize::new(PAR_MIN);
static FOLD_LEN: AtomicUsize = AtomicUsize::new(FOLD_MIN);
static INT_FOLD_LEN: AtomicUsize = AtomicUsize::new(INT_FOLD_MIN);

/// Thread count from `RBQN_THREADS` (if a positive integer) else available parallelism.
pub fn default_threads() -> usize {
    std::env::var("RBQN_THREADS")
        .ok()
        .and_then(|s| s.trim().parse::<usize>().ok())
        .filter(|&n| n >= 1)
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
}

/// Size the global pool. `n == 1` builds no pool: the serial paths run unchanged.
/// Call once, before any evaluation.
pub fn init(n: usize) {
    let n = n.max(1);
    if n > 1 && rayon::ThreadPoolBuilder::new().num_threads(n).build_global().is_err() {
        // A pool already exists (second init): keep whatever size it has.
        THREADS.store(rayon::current_num_threads(), Ordering::Relaxed);
        return;
    }
    THREADS.store(n, Ordering::Relaxed);
    // NOTE: RBQN_PAR_MIN is a measurement/testing override, not a user knob.
    if let Some(m) = std::env::var("RBQN_PAR_MIN").ok().and_then(|s| s.trim().parse::<usize>().ok()) {
        MIN_LEN.store(m.max(1), Ordering::Relaxed);
    }
    if let Some(m) = std::env::var("RBQN_FOLD_MIN").ok().and_then(|s| s.trim().parse::<usize>().ok()) {
        FOLD_LEN.store(m.max(1), Ordering::Relaxed);
        INT_FOLD_LEN.store(m.max(1), Ordering::Relaxed);
    }
}

pub fn threads() -> usize {
    THREADS.load(Ordering::Relaxed)
}

/// Run `f` with every parallel path off, so a caller can time the serial kernels on a
/// multicore host (the cost-model calibration, #167). Only for a thread that is alone in
/// evaluating: the setting is process-wide.
pub fn serially<R>(f: impl FnOnce() -> R) -> R {
    struct Restore(usize);
    impl Drop for Restore {
        fn drop(&mut self) {
            THREADS.store(self.0, Ordering::Relaxed);
        }
    }
    let _restore = Restore(THREADS.swap(1, Ordering::Relaxed));
    f()
}

/// True when a job of `n` elements should take the parallel path.
#[inline]
pub fn go(n: usize) -> bool {
    n >= MIN_LEN.load(Ordering::Relaxed) && THREADS.load(Ordering::Relaxed) > 1
}

/// True when an F64 min/max fold over `n` elements should take the parallel path.
#[inline]
pub fn go_fold(n: usize) -> bool {
    n >= FOLD_LEN.load(Ordering::Relaxed) && THREADS.load(Ordering::Relaxed) > 1
}

/// True when an integer fold over `n` elements should take the parallel path.
#[inline]
pub fn go_int_fold(n: usize) -> bool {
    n >= INT_FOLD_LEN.load(Ordering::Relaxed) && THREADS.load(Ordering::Relaxed) > 1
}

/// Run a parallel region; a panic in a worker becomes a thrown BqnError.
pub(crate) fn guarded<R>(f: impl FnOnce() -> R) -> R {
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(p) => {
            if let Some(e) = p.downcast_ref::<rbqn_core::BqnError>() {
                rbqn_core::error::throw_bqn(e.clone())
            }
            rbqn_core::error::throw("parallel arithmetic: worker panicked")
        }
    }
}

fn chunk_len(n: usize) -> usize {
    // A few chunks per thread for balance, never tiny.
    n.div_ceil(threads() * 4).max(1 << 12)
}

/// `v.iter().map(f).collect()`, chunked when large and `on`. Callers pass
/// `on = input is F64`: integer inputs are widened and squeezed serially, which
/// dominates, and the parallel version measured slower (see PR #104).
pub fn map1<T: Copy + Sync, F: Fn(T) -> f64 + Sync>(on: bool, v: &[T], f: F) -> Vec<f64> {
    if !on || !go(v.len()) {
        return v.iter().map(|&x| f(x)).collect();
    }
    let c = chunk_len(v.len());
    guarded(|| {
        let mut out = vec![0f64; v.len()];
        out.par_chunks_mut(c).zip(v.par_chunks(c)).for_each(|(o, i)| {
            for (o, &x) in o.iter_mut().zip(i) {
                *o = f(x);
            }
        });
        out
    })
}

/// Elementwise `f(w[i], x[i])`; slices have equal length.
pub fn map2<F: Fn(f64, f64) -> f64 + Sync>(on: bool, w: &[f64], x: &[f64], f: F) -> Vec<f64> {
    debug_assert_eq!(w.len(), x.len());
    if !on || !go(w.len()) {
        return w.iter().zip(x.iter()).map(|(&a, &b)| f(a, b)).collect();
    }
    let c = chunk_len(w.len());
    guarded(|| {
        let mut out = vec![0f64; w.len()];
        out.par_chunks_mut(c).zip(w.par_chunks(c)).zip(x.par_chunks(c)).for_each(|((o, a), b)| {
            for ((o, &a), &b) in o.iter_mut().zip(a).zip(b) {
                *o = f(a, b);
            }
        });
        out
    })
}

/// Parallel reduction over chunks of a slice. `chunk` reduces one chunk,
/// `combine` must be associative and exact (the caller guarantees order
/// independence). Only call when `go(v.len())`.
pub fn reduce<T: Sync, R: Send, C: Fn(&[T]) -> R + Sync, M: Fn(R, R) -> R + Sync + Send>(
    v: &[T],
    chunk: C,
    combine: M,
) -> R {
    let c = chunk_len(v.len());
    guarded(|| {
        v.par_chunks(c)
            .map(&chunk)
            .reduce_with(&combine)
            .expect("reduce on empty slice")
    })
}

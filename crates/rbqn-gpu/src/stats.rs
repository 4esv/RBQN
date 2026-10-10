//! Process-wide GPU counters (Relaxed atomics; one increment per event).
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

pub static DISPATCHES: AtomicU64 = AtomicU64::new(0);
pub static SUBMITS: AtomicU64 = AtomicU64::new(0);
pub static READBACKS: AtomicU64 = AtomicU64::new(0);
pub static BYTES_UP: AtomicU64 = AtomicU64::new(0);
pub static BYTES_DOWN: AtomicU64 = AtomicU64::new(0);
/// Device init time in microseconds.
pub static DEVICE_INIT_US: AtomicU64 = AtomicU64::new(0);
pub static PIPELINE_COMPILES: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GpuStats {
    pub dispatches: u64,
    pub submits: u64,
    pub readbacks: u64,
    pub bytes_up: u64,
    pub bytes_down: u64,
    pub device_init_us: u64,
    pub pipeline_compiles: u64,
}

pub fn snapshot() -> GpuStats {
    GpuStats {
        dispatches: DISPATCHES.load(Relaxed),
        submits: SUBMITS.load(Relaxed),
        readbacks: READBACKS.load(Relaxed),
        bytes_up: BYTES_UP.load(Relaxed),
        bytes_down: BYTES_DOWN.load(Relaxed),
        device_init_us: DEVICE_INIT_US.load(Relaxed),
        pipeline_compiles: PIPELINE_COMPILES.load(Relaxed),
    }
}

pub fn reset() {
    for c in [&DISPATCHES, &SUBMITS, &READBACKS, &BYTES_UP, &BYTES_DOWN, &DEVICE_INIT_US, &PIPELINE_COMPILES] {
        c.store(0, Relaxed);
    }
}

#[inline]
pub fn dispatch() {
    DISPATCHES.fetch_add(1, Relaxed);
}

#[inline]
pub fn submit() {
    SUBMITS.fetch_add(1, Relaxed);
}

/// Zero the counters except device init time. Called after the source is compiled so
/// the summary shows the program's own GPU traffic, not the compiler's.
/// Set once the user program starts (after the compiler returns); `RBQN_GPU=force`
/// only applies from then on, so the self-hosted compiler's tiny-array ops stay on CPU.
pub static PROGRAM_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn program_started() -> bool {
    PROGRAM_STARTED.load(Relaxed)
}

pub fn reset_run() {
    PROGRAM_STARTED.store(true, Relaxed);
    let init = DEVICE_INIT_US.load(Relaxed);
    reset();
    DEVICE_INIT_US.store(init, Relaxed);
}

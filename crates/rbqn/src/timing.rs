//! Env-gated startup stage timer: `RBQN_TIMING=1` prints ms per stage to stderr.

use std::sync::OnceLock;
use std::time::Instant;

fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("RBQN_TIMING").is_some())
}

/// Print the time since `*t` under `stage` and reset `*t` to now.
pub fn lap(t: &mut Instant, stage: &str) {
    if enabled() {
        let now = Instant::now();
        eprintln!("[timing] {stage:<24} {:7.3} ms", (now - *t).as_secs_f64() * 1e3);
        *t = now;
    }
}

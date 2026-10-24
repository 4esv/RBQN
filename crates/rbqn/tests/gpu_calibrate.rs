//! Cost-model calibration (#167): the first device use measures the model's constants and
//! caches them under `RBQN_CACHE_DIR`; the next run reads them back.
#![cfg(feature = "gpu")]

use std::path::PathBuf;
use std::process::Command;

fn run(dir: &PathBuf, env: &[(&str, &str)], expr: &str) -> (String, String) {
    let mut c = Command::new(env!("CARGO_BIN_EXE_rbqn"));
    c.args(["-p", expr]).env("RBQN_CACHE_DIR", dir);
    for (k, v) in env {
        c.env(k, v);
    }
    let o = c.output().expect("failed to run rbqn");
    (String::from_utf8_lossy(&o.stdout).trim().to_string(), String::from_utf8_lossy(&o.stderr).to_string())
}

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("rbqn-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

/// The `[gpu] model k=v ...` line (the constants), or "".
fn constants(log: &str) -> String {
    log.lines().find(|l| l.starts_with("[gpu] model gpu_init_ms=")).unwrap_or("").to_string()
}

#[test]
fn calibrate_prints_constants_writes_cache_and_second_run_reads_it() {
    let probe = scratch("probe");
    if run(&probe, &[], "•gpu.Available @").0 != "1" {
        eprintln!("no GPU device, skipping");
        let _ = std::fs::remove_dir_all(&probe);
        return;
    }
    let _ = std::fs::remove_dir_all(&probe);
    let dir = scratch("cal");

    let (out, log) = run(&dir, &[("RBQN_GPU_CALIBRATE", "1"), ("RBQN_GPU_DEBUG", "1")], "•gpu.Available @");
    assert_eq!(out, "1");
    assert!(log.contains("[gpu] model measured"), "{log}");
    assert!(log.contains(&format!("cache={}", dir.join("gpu-model.txt").display())), "{log}");
    let first = constants(&log);
    for k in [
        "gpu_init_ms", "gpu_fixed_ms", "gpu_map_ns", "gpu_op_ns", "gpu_up_ns_per_b", "gpu_down_ns_per_b",
        "gpu_scan_ns", "gpu_sort_ns", "gpu_matmul_ns_per_mac", "cpu_op_ns", "cpu_leaf_ns", "cpu_out_ns",
        "cpu_fold_ns", "cpu_scan_ns", "cpu_sort_ns", "cpu_matmul_ns_per_mac", "cpu_matmul_thread_exp",
    ] {
        assert!(first.contains(&format!(" {k}=")), "{k} missing from {first:?}");
    }
    // Calibration's own dispatches stay out of the log and the counters the program sees.
    assert!(!log.contains("[gpu] matmul"), "{log}");
    let file = std::fs::read_to_string(dir.join("gpu-model.txt")).expect("cache file written");
    assert!(file.starts_with("rbqn-gpu-model 1\n") && file.contains("\nentry\t"), "{file}");

    let (out, log) = run(&dir, &[("RBQN_GPU_DEBUG", "1")], "•gpu.Available @");
    assert_eq!(out, "1");
    assert!(log.contains("[gpu] model cached"), "{log}");
    assert!(!log.contains("[gpu] model measured"), "{log}");
    assert_eq!(constants(&log), first, "the second run uses the cached constants");

    // RBQN_GPU_CALIBRATE=1 measures again even though the cache holds this adapter.
    let (_, log) = run(&dir, &[("RBQN_GPU_CALIBRATE", "1"), ("RBQN_GPU_DEBUG", "1")], "•gpu.Available @");
    assert!(log.contains("[gpu] model measured"), "{log}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn off_never_calibrates_or_touches_the_cache() {
    let dir = scratch("off");
    let (out, log) = run(&dir, &[("RBQN_GPU", "off"), ("RBQN_GPU_DEBUG", "1")], "•gpu.Available @");
    assert_eq!(out, "0");
    assert!(!log.contains("[gpu] model"), "{log}");
    assert!(!dir.exists());
}

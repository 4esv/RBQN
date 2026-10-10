//! End-to-end check that large arrays actually reach the GPU (issue #10).
//! Runs the release binary with RBQN_GPU_DEBUG=1 and asserts on the dispatch
//! log. Skips when no GPU device initializes.

use std::process::Command;

fn run(expr: &str) -> (String, String) {
    let output = Command::new("cargo")
        .args(["run", "-p", "rbqn", "-q", "--release", "--", "-p", expr])
        .env("RBQN_GPU_DEBUG", "1")
        .output()
        .expect("failed to run rbqn");
    (
        String::from_utf8_lossy(&output.stdout).trim().to_string(),
        String::from_utf8_lossy(&output.stderr).to_string(),
    )
}

fn check(expr: &str, log: &str, expected: &str) {
    let (stdout, stderr) = run(expr);
    if !stderr.contains("[gpu] initializing device") {
        eprintln!("no GPU device, skipping {expr}");
        return;
    }
    assert!(!stderr.contains("panicked"), "{expr}: {stderr}");
    assert!(stderr.contains(log), "{expr}: expected log {log:?}, got:\n{stderr}");
    assert_eq!(stdout, expected, "{expr}");
}

#[test]
fn reduce_100m_runs_on_gpu() {
    check("×´ 100000000⥊1", "[gpu] reduce_mul 100000000 elements (i32)", "1");
}

#[test]
fn arith_40m_runs_on_gpu() {
    check(
        "+´ (↕40000000)+(↕40000000)",
        "[gpu] arith 40000000 elements (i32)",
        "1599999960000000",
    );
}

#[test]
fn scan_40m_runs_on_gpu() {
    check("+´+`40000000⥊1", "[gpu] scan_add 40000000 elements (i32)", "800000020000000");
}
